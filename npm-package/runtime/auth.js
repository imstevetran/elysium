/**
 * Elysium Auth Runtime — Session, JWT, Passkey, OAuth2, Authorization, Multi-tenant.
 *
 * Provides a comprehensive auth primitive for Elysium programs.
 * All functions accept/return strings (JSON-encoded where necessary).
 *
 * Environment variables:
 *   AUTH_JWT_SECRET      — Secret key for JWT signing/verification (default: "elysium-dev-secret")
 *   AUTH_JWT_ISSUER      — JWT issuer claim (default: "elysium")
 *   AUTH_SESSION_TTL_MS  — Session TTL in ms (default: 3600000 = 1 hour)
 *   AUTH_TENANT_ID       — Default tenant ID (default: "default")
 *
 * Exported API (called via desugared `__auth_*` names):
 *  - jwtSign(payload, expiresIn)       → String  Sign a JWT (payload is JSON, expiresIn like "1h")
 *  - jwtVerify(token)                  → String  Verify a JWT, returns decoded payload or error JSON
 *  - jwtDecode(token)                  → String  Decode a JWT without verification
 *  - createSession(userId, data)       → String  Create a session, returns session ID
 *  - getSession(sessionId)             → String  Get session data, returns JSON or error
 *  - destroySession(sessionId)         → Nil     Destroy a session
 *  - hashPassword(password)            → String  Hash a password with bcrypt-style hash
 *  - verifyPassword(password, hash)    → String  Verify a password against a hash
 *  - checkPermission(user, permission) → String  Check if user has a permission (JSON result)
 *  - hasRole(user, role)               → String  Check if user has a role (JSON result)
 *  - hasScope(token, scope)            → String  Check if a JWT token has a scope (JSON result)
 *  - oauth2Authorize(clientId, redirectUri, scope) → String  Generate OAuth2 authorization URL
 *  - oauth2Token(code, clientId, clientSecret)     → String  Exchange auth code for tokens
 *  - oauth2Refresh(refreshToken, clientId)         → String  Refresh an access token
 *  - passkeyRegister(userId, userName)             → String  Generate passkey registration options
 *  - passkeyAuthenticate(userId)                   → String  Generate passkey authentication options
 *  - tenantContext(tenantId)           → String  Set and return tenant context info
 *  - getTenant()                      → String  Get current tenant ID
 *  - listTenants()                    → String  List all tenants (JSON array)
 *  - createTenant(tenantId, config)   → String  Create a new tenant
 */

// ─── Configuration ────────────────────────────────────────────────────────────

const JWT_SECRET = process.env.AUTH_JWT_SECRET || 'elysium-dev-secret';
const JWT_ISSUER = process.env.AUTH_JWT_ISSUER || 'elysium';
const SESSION_TTL = parseInt(process.env.AUTH_SESSION_TTL_MS || '3600000', 10);
const DEFAULT_TENANT = process.env.AUTH_TENANT_ID || 'default';

// ─── Internal State ───────────────────────────────────────────────────────────

const sessions = new Map();        // sessionId → { userId, data, createdAt, tenantId }
const tenants = new Map();         // tenantId → { config }
const userRoles = new Map();       // userId → string[] (roles)
const userPermissions = new Map(); // userId → string[] (permissions)

// Initialize default tenant
tenants.set(DEFAULT_TENANT, { config: '{}' });

// ─── Utilities ────────────────────────────────────────────────────────────────

function base64UrlEncode(str) {
  const bytes = Buffer.from(str);
  return bytes.toString('base64').replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
}

function base64UrlDecode(str) {
  str = str.replace(/-/g, '+').replace(/_/g, '/');
  while (str.length % 4) str += '=';
  return Buffer.from(str, 'base64').toString('utf-8');
}

function hmacSha256(secret, data) {
  const crypto = require('crypto');
  return crypto.createHmac('sha256', secret).update(data).digest('base64');
}

function currentTimestamp() {
  return Math.floor(Date.now() / 1000);
}

function generateId() {
  const crypto = require('crypto');
  return crypto.randomUUID();
}

function simpleHash(str) {
  const crypto = require('crypto');
  return crypto.createHash('sha256').update(str).digest('hex');
}

function jsonOk(data) {
  return JSON.stringify({ ok: true, data });
}

function jsonError(message) {
  return JSON.stringify({ ok: false, error: message });
}

function parseExpiresIn(expiresIn) {
  if (!expiresIn || expiresIn === '') return 3600;
  const match = expiresIn.match(/^(\d+)\s*(s|m|h|d)?$/);
  if (!match) return 3600;
  const num = parseInt(match[1], 10);
  switch (match[2]) {
    case 's': return num;
    case 'm': return num * 60;
    case 'h': return num * 3600;
    case 'd': return num * 86400;
    default: return num;
  }
}

// ─── JWT ──────────────────────────────────────────────────────────────────────

function jwtSign(payload, expiresIn) {
  try {
    const header = { alg: 'HS256', typ: 'JWT' };
    const now = currentTimestamp();
    const ttl = parseExpiresIn(expiresIn);
    const parsedPayload = JSON.parse(payload);
    const jwtPayload = {
      ...parsedPayload,
      iss: JWT_ISSUER,
      iat: now,
      exp: now + ttl,
    };

    const headerB64 = base64UrlEncode(JSON.stringify(header));
    const payloadB64 = base64UrlEncode(JSON.stringify(jwtPayload));
    const signature = hmacSha256(JWT_SECRET, `${headerB64}.${payloadB64}`);
    const sigB64 = base64UrlEncode(signature);

    return `${headerB64}.${payloadB64}.${sigB64}`;
  } catch (e) {
    return jsonError(`jwtSign: ${e.message}`);
  }
}

function jwtVerify(token) {
  try {
    const parts = token.split('.');
    if (parts.length !== 3) return jsonError('jwtVerify: invalid token format');

    const [, payloadB64, sigB64] = parts;
    const headerB64 = parts[0];

    // Verify signature
    const expectedSig = hmacSha256(JWT_SECRET, `${headerB64}.${payloadB64}`);
    const expectedSigB64 = base64UrlEncode(expectedSig);
    // Constant-time comparison using HMAC comparison
    const actualSigB64 = sigB64;
    const crypto = require('crypto');
    const expected = crypto.createHash('sha256').update(expectedSigB64).digest();
    const actual = crypto.createHash('sha256').update(actualSigB64).digest();
    if (!expected.equals(actual)) return jsonError('jwtVerify: invalid signature');

    // Decode payload
    const decoded = JSON.parse(base64UrlDecode(payloadB64));

    // Check expiration
    const now = currentTimestamp();
    if (decoded.exp && decoded.exp < now) return jsonError('jwtVerify: token expired');

    // Check not before
    if (decoded.nbf && decoded.nbf > now) return jsonError('jwtVerify: token not yet valid');

    return jsonOk(decoded);
  } catch (e) {
    return jsonError(`jwtVerify: ${e.message}`);
  }
}

function jwtDecode(token) {
  try {
    const parts = token.split('.');
    if (parts.length !== 3) return jsonError('jwtDecode: invalid token format');
    const decoded = JSON.parse(base64UrlDecode(parts[1]));
    return jsonOk(decoded);
  } catch (e) {
    return jsonError(`jwtDecode: ${e.message}`);
  }
}

// ─── Session Management ───────────────────────────────────────────────────────

function createSession(userId, data) {
  try {
    const sessionId = generateId();
    const parsedData = (() => { try { return JSON.parse(data); } catch (_) { return { data }; } })();
    sessions.set(sessionId, {
      userId,
      data: parsedData,
      createdAt: Date.now(),
      tenantId: DEFAULT_TENANT,
    });
    return jsonOk({ sessionId, userId, ...parsedData });
  } catch (e) {
    return jsonError(`createSession: ${e.message}`);
  }
}

function getSession(sessionId) {
  try {
    const session = sessions.get(sessionId);
    if (!session) return jsonError('getSession: session not found');

    // Check TTL
    if (Date.now() - session.createdAt > SESSION_TTL) {
      sessions.delete(sessionId);
      return jsonError('getSession: session expired');
    }

    return jsonOk({ sessionId, userId: session.userId, ...session.data, tenantId: session.tenantId });
  } catch (e) {
    return jsonError(`getSession: ${e.message}`);
  }
}

function destroySession(sessionId) {
  sessions.delete(sessionId);
  return null;
}

// ─── Password Hashing ─────────────────────────────────────────────────────────

function hashPassword(password) {
  try {
    // Simple salted SHA-256 hash (in production, use bcrypt/argon2 via env)
    const crypto = require('crypto');
    const salt = crypto.randomBytes(16).toString('hex');
    const hash = crypto.pbkdf2Sync(password, salt, 10000, 32, 'sha256').toString('hex');
    return `${salt}:${hash}`;
  } catch (e) {
    return jsonError(`hashPassword: ${e.message}`);
  }
}

function verifyPassword(password, hash) {
  try {
    const parts = hash.split(':');
    if (parts.length !== 2) return jsonError('verifyPassword: invalid hash format');
    const [salt, expectedHash] = parts;
    const crypto = require('crypto');
    const computedHash = crypto.pbkdf2Sync(password, salt, 10000, 32, 'sha256').toString('hex');
    return jsonOk({ valid: computedHash === expectedHash });
  } catch (e) {
    return jsonError(`verifyPassword: ${e.message}`);
  }
}

// ─── Authorization ────────────────────────────────────────────────────────────

function checkPermission(user, permission) {
  try {
    const perms = userPermissions.get(user) || [];
    const result = perms.includes(permission);
    return jsonOk({ permitted: result, user, permission });
  } catch (e) {
    return jsonError(`checkPermission: ${e.message}`);
  }
}

function hasRole(user, role) {
  try {
    const roles = userRoles.get(user) || [];
    const result = roles.includes(role);
    return jsonOk({ hasRole: result, user, role });
  } catch (e) {
    return jsonError(`hasRole: ${e.message}`);
  }
}

function hasScope(token, scope) {
  try {
    const parts = token.split('.');
    if (parts.length !== 3) return jsonError('hasScope: invalid token');
    const decoded = JSON.parse(base64UrlDecode(parts[1]));
    const scopes = decoded.scope ? decoded.scope.split(' ') : [];
    const result = scopes.includes(scope);
    return jsonOk({ hasScope: result, token: decoded.sub, scope });
  } catch (e) {
    return jsonError(`hasScope: ${e.message}`);
  }
}

// ─── OAuth2 ───────────────────────────────────────────────────────────────────

// In-memory OAuth2 state (for development/testing)
const oauthClients = new Map();   // clientId → { clientSecret, redirectUris }
const oauthCodes = new Map();     // code → { clientId, redirectUri, scope, expiresAt }
const oauthTokens = new Map();    // accessToken → { clientId, userId, scope, expiresAt, refreshToken }
const oauthRefreshTokens = new Map(); // refreshToken → { clientId, accessToken }

function oauth2Authorize(clientId, redirectUri, scope) {
  try {
    // Validate client
    if (!oauthClients.has(clientId)) {
      // Auto-register for development
      oauthClients.set(clientId, { clientSecret: 'dev-secret', redirectUris: [redirectUri] });
    }

    // Generate authorization code
    const code = generateId();
    oauthCodes.set(code, {
      clientId,
      redirectUri,
      scope,
      expiresAt: Date.now() + 600000, // 10 min
    });

    const params = new URLSearchParams({ code, state: 'elysium-state' });
    return jsonOk({ authorizationUrl: `${redirectUri}?${params.toString()}`, code });
  } catch (e) {
    return jsonError(`oauth2Authorize: ${e.message}`);
  }
}

function oauth2Token(code, clientId, clientSecret) {
  try {
    const authCode = oauthCodes.get(code);
    if (!authCode) return jsonError('oauth2Token: invalid authorization code');
    if (authCode.clientId !== clientId) return jsonError('oauth2Token: client mismatch');
    if (Date.now() > authCode.expiresAt) return jsonError('oauth2Token: authorization code expired');

    // Validate client secret
    const client = oauthClients.get(clientId);
    if (!client || client.clientSecret !== clientSecret) return jsonError('oauth2Token: invalid client credentials');

    // Generate tokens
    const accessToken = generateId();
    const refreshToken = generateId();
    const expiresIn = 3600;

    oauthTokens.set(accessToken, {
      clientId,
      userId: 'oauth-user',
      scope: authCode.scope,
      expiresAt: Date.now() + expiresIn * 1000,
      refreshToken,
    });

    oauthRefreshTokens.set(refreshToken, { clientId, accessToken });

    // Clean up used auth code
    oauthCodes.delete(code);

    return jsonOk({
      accessToken,
      tokenType: 'Bearer',
      expiresIn,
      refreshToken,
      scope: authCode.scope,
    });
  } catch (e) {
    return jsonError(`oauth2Token: ${e.message}`);
  }
}

function oauth2Refresh(refreshToken, clientId) {
  try {
    const stored = oauthRefreshTokens.get(refreshToken);
    if (!stored) return jsonError('oauth2Refresh: invalid refresh token');
    if (stored.clientId !== clientId) return jsonError('oauth2Refresh: client mismatch');

    // Revoke old access token
    oauthTokens.delete(stored.accessToken);

    // Generate new tokens
    const newAccessToken = generateId();
    const newRefreshToken = generateId();
    const oldToken = oauthTokens.get(stored.accessToken) || { scope: '', userId: 'oauth-user' };
    const expiresIn = 3600;

    oauthTokens.set(newAccessToken, {
      clientId,
      userId: oldToken.userId,
      scope: oldToken.scope,
      expiresAt: Date.now() + expiresIn * 1000,
      refreshToken: newRefreshToken,
    });

    oauthRefreshTokens.delete(refreshToken);
    oauthRefreshTokens.set(newRefreshToken, { clientId, accessToken: newAccessToken });

    return jsonOk({
      accessToken: newAccessToken,
      tokenType: 'Bearer',
      expiresIn,
      refreshToken: newRefreshToken,
    });
  } catch (e) {
    return jsonError(`oauth2Refresh: ${e.message}`);
  }
}

// ─── Passkey (WebAuthn) ──────────────────────────────────────────────────────

// In-memory passkey registration state
const passkeyRegistrations = new Map(); // userId → { credentialId, publicKey, counter }

function passkeyRegister(userId, userName) {
  try {
    const crypto = require('crypto');

    // Generate a mock credential ID and public key
    const credentialId = base64UrlEncode(crypto.randomBytes(32));
    const challenge = base64UrlEncode(crypto.randomBytes(32));
    const publicKey = base64UrlEncode(crypto.randomBytes(65));

    const registrationOptions = {
      challenge,
      rp: { name: 'Elysium Auth', id: 'elysium.local' },
      user: {
        id: base64UrlEncode(Buffer.from(userId)),
        name: userName,
        displayName: userName,
      },
      pubKeyCredParams: [{ type: 'public-key', alg: -7 }],
      timeout: 60000,
      attestation: 'none',
      excludeCredentials: [],
      authenticatorSelection: {
        residentKey: 'preferred',
        userVerification: 'preferred',
      },
    };

    return jsonOk({ registrationOptions, credentialId, publicKey });
  } catch (e) {
    return jsonError(`passkeyRegister: ${e.message}`);
  }
}

function passkeyAuthenticate(userId) {
  try {
    const crypto = require('crypto');
    const registration = passkeyRegistrations.get(userId);
    const challenge = base64UrlEncode(crypto.randomBytes(32));

    const authOptions = {
      challenge,
      timeout: 60000,
      rpId: 'elysium.local',
      allowCredentials: registration
        ? [{ type: 'public-key', id: registration.credentialId }]
        : [],
      userVerification: 'preferred',
    };

    return jsonOk({ authenticationOptions: authOptions, challenge });
  } catch (e) {
    return jsonError(`passkeyAuthenticate: ${e.message}`);
  }
}

// ─── Multi-tenant ─────────────────────────────────────────────────────────────

function tenantContext(tenantId) {
  try {
    if (!tenants.has(tenantId)) {
      tenants.set(tenantId, { config: '{}' });
    }
    const config = tenants.get(tenantId);
    return jsonOk({ tenantId, config: config.config });
  } catch (e) {
    return jsonError(`tenantContext: ${e.message}`);
  }
}

function getTenant() {
  return jsonOk({ tenantId: DEFAULT_TENANT });
}

function listTenants() {
  try {
    const tenantList = Array.from(tenants.keys());
    return jsonOk(tenantList);
  } catch (e) {
    return jsonError(`listTenants: ${e.message}`);
  }
}

function createTenant(tenantId, config) {
  try {
    if (tenants.has(tenantId)) return jsonError(`createTenant: tenant '${tenantId}' already exists`);
    const parsedConfig = (() => { try { return JSON.parse(config); } catch (_) { return { config }; } })();
    tenants.set(tenantId, { config: JSON.stringify(parsedConfig) });
    return jsonOk({ tenantId, config: parsedConfig });
  } catch (e) {
    return jsonError(`createTenant: ${e.message}`);
  }
}

// ─── Role/Permission Management ───────────────────────────────────────────────

function grantRole(user, role) {
  try {
    const roles = userRoles.get(user) || [];
    if (!roles.includes(role)) {
      roles.push(role);
      userRoles.set(user, roles);
    }
    return jsonOk({ user, roles });
  } catch (e) {
    return jsonError(`grantRole: ${e.message}`);
  }
}

function grantPermission(user, permission) {
  try {
    const perms = userPermissions.get(user) || [];
    if (!perms.includes(permission)) {
      perms.push(permission);
      userPermissions.set(user, perms);
    }
    return jsonOk({ user, permissions: perms });
  } catch (e) {
    return jsonError(`grantPermission: ${e.message}`);
  }
}

function revokeRole(user, role) {
  try {
    const roles = userRoles.get(user) || [];
    userRoles.set(user, roles.filter(r => r !== role));
    return jsonOk({ user, roles: userRoles.get(user) });
  } catch (e) {
    return jsonError(`revokeRole: ${e.message}`);
  }
}

function revokePermission(user, permission) {
  try {
    const perms = userPermissions.get(user) || [];
    userPermissions.set(user, perms.filter(p => p !== permission));
    return jsonOk({ user, permissions: userPermissions.get(user) });
  } catch (e) {
    return jsonError(`revokePermission: ${e.message}`);
  }
}

// ─── API Key Auth ─────────────────────────────────────────────────────────────

const apiKeys = new Map(); // keyHash → { userId, tenantId, permissions[] }

function generateApiKey(userId) {
  try {
    const crypto = require('crypto');
    const rawKey = `ely_${crypto.randomBytes(24).toString('hex')}`;
    const keyHash = simpleHash(rawKey);
    apiKeys.set(keyHash, { userId, tenantId: DEFAULT_TENANT, permissions: [] });
    return jsonOk({ apiKey: rawKey, userId });
  } catch (e) {
    return jsonError(`generateApiKey: ${e.message}`);
  }
}

function validateApiKey(apiKey) {
  try {
    const keyHash = simpleHash(apiKey);
    const record = apiKeys.get(keyHash);
    if (!record) return jsonError('validateApiKey: invalid API key');
    return jsonOk({ valid: true, userId: record.userId, tenantId: record.tenantId });
  } catch (e) {
    return jsonError(`validateApiKey: ${e.message}`);
  }
}

// ─── RBAC (Role-Based Access Control) ────────────────────────────────────────

function checkAccess(userId, resource, action) {
  try {
    const roles = userRoles.get(userId) || [];
    const perms = userPermissions.get(userId) || [];

    // Define role → permission mappings
    const rolePermissions = {
      admin: ['*'],
      editor: ['read', 'write', 'update'],
      viewer: ['read'],
    };

    // Check if user has admin role (wildcard access)
    if (roles.some(r => (rolePermissions[r] || []).includes('*'))) {
      return jsonOk({ granted: true, userId, resource, action, via: 'admin_role' });
    }

    // Check direct permissions
    if (perms.includes(`${resource}:${action}`) || perms.includes('*')) {
      return jsonOk({ granted: true, userId, resource, action, via: 'permission' });
    }

    // Check role-based permissions
    for (const role of roles) {
      const allowedActions = rolePermissions[role] || [];
      if (allowedActions.includes(action) || allowedActions.includes('*')) {
        return jsonOk({ granted: true, userId, resource, action, via: `role:${role}` });
      }
    }

    return jsonOk({ granted: false, userId, resource, action, via: 'denied' });
  } catch (e) {
    return jsonError(`checkAccess: ${e.message}`);
  }
}

function setRoles(userId, rolesJson) {
  try {
    const rolesArr = JSON.parse(rolesJson);
    if (!Array.isArray(rolesArr)) return jsonError('setRoles: roles must be a JSON array');
    userRoles.set(userId, rolesArr);
    return jsonOk({ userId, roles: rolesArr });
  } catch (e) {
    return jsonError(`setRoles: ${e.message}`);
  }
}

function setPermissions(userId, permissionsJson) {
  try {
    const permsArr = JSON.parse(permissionsJson);
    if (!Array.isArray(permsArr)) return jsonError('setPermissions: permissions must be a JSON array');
    userPermissions.set(userId, permsArr);
    return jsonOk({ userId, permissions: permsArr });
  } catch (e) {
    return jsonError(`setPermissions: ${e.message}`);
  }
}

// ─── Exports ──────────────────────────────────────────────────────────────────

module.exports = {
  // JWT
  jwtSign,
  jwtVerify,
  jwtDecode,
  // Session
  createSession,
  getSession,
  destroySession,
  // Password
  hashPassword,
  verifyPassword,
  // Authorization
  checkPermission,
  hasRole,
  hasScope,
  // OAuth2
  oauth2Authorize,
  oauth2Token,
  oauth2Refresh,
  // Passkey
  passkeyRegister,
  passkeyAuthenticate,
  // Multi-tenant
  tenantContext,
  getTenant,
  listTenants,
  createTenant,
  // Role/Permission management
  grantRole,
  grantPermission,
  revokeRole,
  revokePermission,
  // API Key
  generateApiKey,
  validateApiKey,
  // RBAC
  checkAccess,
  setRoles,
  setPermissions,
};
