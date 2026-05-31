/**
 * JSON runtime functions for Elysium.
 * Standalone C implementation providing JSON string building and parsing.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <ctype.h>

/* Build a JSON object from key-value pairs.
 * The arguments are: key1, val1, key2, val2, ..., NULL
 */
char* __ely_json_buildObject(const char* key1, ...) {
    size_t cap = 256;
    char* result = malloc(cap);
    if (!result) return NULL;
    result[0] = '{';
    size_t pos = 1;

    va_list args;
    va_start(args, key1);
    const char* k = key1;
    int first = 1;
    while (k) {
        const char* v = va_arg(args, const char*);
        if (!v) break;

        size_t needed = pos + strlen(k) + strlen(v) + 8;
        if (needed >= cap) {
            cap = needed + 256;
            char* newres = realloc(result, cap);
            if (!newres) { free(result); va_end(args); return NULL; }
            result = newres;
        }

        if (!first) {
            result[pos++] = ',';
        }
        first = 0;

        /* Append: "key":"value" */
        result[pos++] = '"';
        size_t klen = strlen(k);
        memcpy(result + pos, k, klen);
        pos += klen;
        result[pos++] = '"';
        result[pos++] = ':';
        result[pos++] = '"';
        /* Escape value (simple: replace " with \") */
        for (const char* p = v; *p; p++) {
            if (*p == '"') {
                if (pos + 2 >= cap) {
                    cap += 256;
                    char* newres = realloc(result, cap);
                    if (!newres) { free(result); va_end(args); return NULL; }
                    result = newres;
                }
                result[pos++] = '\\';
            }
            if (pos + 1 >= cap) {
                cap += 256;
                char* newres = realloc(result, cap);
                if (!newres) { free(result); va_end(args); return NULL; }
                result = newres;
            }
            result[pos++] = *p;
        }
        result[pos++] = '"';

        k = va_arg(args, const char*);
    }
    va_end(args);

    result[pos++] = '}';
    result[pos] = '\0';
    return result;
}

/* Build a JSON array from string values.
 * Varargs: val1, val2, ..., NULL
 */
char* __ely_json_buildArray(const char* val1, ...) {
    size_t cap = 256;
    char* result = malloc(cap);
    if (!result) return NULL;
    result[0] = '[';
    size_t pos = 1;

    va_list args;
    va_start(args, val1);
    const char* v = val1;
    int first = 1;
    while (v) {
        size_t needed = pos + strlen(v) + 4;
        if (needed >= cap) {
            cap = needed + 256;
            char* newres = realloc(result, cap);
            if (!newres) { free(result); va_end(args); return NULL; }
            result = newres;
        }

        if (!first) result[pos++] = ',';
        first = 0;
        result[pos++] = '"';
        /* Escape */
        for (const char* p = v; *p; p++) {
            if (*p == '"') {
                if (pos + 2 >= cap) {
                    cap += 256;
                    char* newres = realloc(result, cap);
                    if (!newres) { free(result); va_end(args); return NULL; }
                    result = newres;
                }
                result[pos++] = '\\';
            }
            if (pos + 1 >= cap) {
                cap += 256;
                char* newres = realloc(result, cap);
                if (!newres) { free(result); va_end(args); return NULL; }
                result = newres;
            }
            result[pos++] = *p;
        }
        result[pos++] = '"';
        v = va_arg(args, const char*);
    }
    va_end(args);

    result[pos++] = ']';
    result[pos] = '\0';
    return result;
}

/* Build a message object: {"role":"role","content":"content"} */
char* __ely_json_buildMessage(const char* role, const char* content) {
    size_t rlen = strlen(role);
    size_t clen = strlen(content);
    size_t cap = rlen + clen + 64;
    char* result = malloc(cap);
    if (!result) return NULL;

    size_t esc_count = 0;
    for (const char* p = content; *p; p++) if (*p == '"') esc_count++;
    size_t esc_cap = cap + esc_count + 16;
    char* escaped = malloc(esc_cap);
    if (!escaped) { free(result); return NULL; }
    size_t epos = 0;
    for (const char* p = content; *p; p++) {
        if (*p == '"') { escaped[epos++] = '\\'; }
        escaped[epos++] = *p;
    }
    escaped[epos] = '\0';

    snprintf(result, cap, "{\"role\":\"%s\",\"content\":\"%s\"}", role, escaped);
    free(escaped);
    return result;
}

/* "Parse" JSON — just strdup (the string IS the handle) */
char* __ely_json_parse(const char* str) {
    return strdup(str);
}

/* Get a value by key path from a JSON string.
 * Supports simple dot paths like "choices.0.message.content"
 * Returns the string value (without quotes) or empty string.
 * This is a SIMPLE implementation — not a full JSON parser.
 */
char* __ely_json_get(const char* json_str, const char* key_path) {
    if (!json_str || !key_path) return strdup("");

    char* work = strdup(json_str);
    if (!work) return strdup("");

    char* result = NULL;

    char* path_copy = strdup(key_path);
    if (!path_copy) { free(work); return strdup(""); }

    char* cursor = work;
    char* token = strtok(path_copy, ".");
    int found = 1;

    while (token && found) {
        char search_key[1024];
        snprintf(search_key, sizeof(search_key), "\"%s\"", token);

        char* key_pos = strstr(cursor, search_key);
        if (!key_pos) {
            found = 0;
            break;
        }

        key_pos += strlen(search_key);

        while (*key_pos && (*key_pos == ' ' || *key_pos == '\t' || *key_pos == '\n')) key_pos++;
        if (*key_pos == ':') key_pos++;
        while (*key_pos && (*key_pos == ' ' || *key_pos == '\t' || *key_pos == '\n')) key_pos++;

        int is_array_index = 1;
        for (const char* p = token; *p; p++) {
            if (!isdigit((unsigned char)*p)) { is_array_index = 0; break; }
        }

        if (is_array_index) {
            int idx = atoi(token);
            if (*key_pos == '[') key_pos++;
            int depth = 0;
            int current = 0;
            while (*key_pos && current < idx) {
                if (*key_pos == '{' || *key_pos == '[') depth++;
                else if (*key_pos == '}' || *key_pos == ']') depth--;
                else if (*key_pos == ',' && depth == 0) current++;
                key_pos++;
            }
        }

        cursor = key_pos;
        token = strtok(NULL, ".");
    }

    if (found && cursor) {
        while (*cursor && (*cursor == ' ' || *cursor == '\t' || *cursor == '\n')) cursor++;

        if (*cursor == '"') {
            cursor++;
            char* end = cursor;
            while (*end) {
                if (*end == '\\') { end += 2; continue; }
                if (*end == '"') break;
                end++;
            }
            size_t len = end - cursor;
            result = malloc(len + 1);
            if (result) {
                memcpy(result, cursor, len);
                result[len] = '\0';
            }
        } else if (*cursor == '{' || *cursor == '[') {
            char open = *cursor;
            char close = (open == '{') ? '}' : ']';
            int depth = 0;
            char* end = cursor;
            while (*end) {
                if (*end == open) depth++;
                else if (*end == close) { depth--; if (depth == 0) { end++; break; } }
                end++;
            }
            size_t len = end - cursor;
            result = malloc(len + 1);
            if (result) {
                memcpy(result, cursor, len);
                result[len] = '\0';
            }
        } else {
            char* end = cursor;
            while (*end && *end != ',' && *end != '}' && *end != ']' && *end != '\0') end++;
            size_t len = end - cursor;
            result = malloc(len + 1);
            if (result) {
                memcpy(result, cursor, len);
                result[len] = '\0';
            }
        }
    }

    if (!result) result = strdup("");
    free(work);
    free(path_copy);
    return result;
}

/* Free a JSON handle (free the string) */
void __ely_json_free(char* handle) {
    if (handle) free(handle);
}
