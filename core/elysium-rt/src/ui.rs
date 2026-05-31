// Reactive UI engine for Elysium 2.0
// Component system with virtual DOM diffing.

use std::any::Any;
use std::collections::HashMap;

/// A view is an immutable description of a UI element.
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    Text { content: String, style: Style },
    Button { label: String },
    TextField { value: String },
    Image { src: String, width: f64, height: f64 },
    Column { children: Vec<View>, padding: f64 },
    Row { children: Vec<View> },
    ScrollView { axis: Axis, child: Box<View> },
    ListView { items: Vec<String> },
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub color: Option<String>,
    pub font_size: Option<f64>,
    pub bold: Option<bool>,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            color: None,
            font_size: None,
            bold: None,
        }
    }
}

/// A reactive component state container.
pub struct ComponentState {
    pub name: String,
    pub state: HashMap<String, Box<dyn Any>>,
}

impl ComponentState {
    pub fn new(name: &str) -> Self {
        ComponentState {
            name: name.to_string(),
            state: HashMap::new(),
        }
    }

    pub fn set<T: 'static>(&mut self, key: &str, value: T) {
        self.state.insert(key.to_string(), Box::new(value));
    }

    pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
        self.state.get(key).and_then(|v| v.downcast_ref::<T>())
    }
}

/// Simple virtual DOM diff engine.
/// Returns a list of patches to apply to the real DOM.
#[derive(Debug, Clone)]
pub enum Patch {
    Replace { old: View, new: View },
    UpdateText { index: usize, new_text: String },
    UpdateStyle { index: usize, style: Style },
    AppendChild { parent_index: usize, child: View },
    RemoveChild { index: usize },
    Noop,
}

pub fn diff(old: &[View], new: &[View]) -> Vec<Patch> {
    let mut patches = Vec::new();
    let max_len = old.len().max(new.len());

    for i in 0..max_len {
        match (old.get(i), new.get(i)) {
            (None, Some(new_view)) => {
                patches.push(Patch::AppendChild {
                    parent_index: 0,
                    child: new_view.clone(),
                });
            }
            (Some(_), None) => {
                patches.push(Patch::RemoveChild { index: i });
            }
            (Some(old_view), Some(new_view)) => {
                if old_view != new_view {
                    patches.push(Patch::Replace {
                        old: old_view.clone(),
                        new: new_view.clone(),
                    });
                }
            }
            (None, None) => {}
        }
    }

    patches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_no_changes() {
        let views = vec![View::Text {
            content: "hello".into(),
            style: Style::default(),
        }];
        let patches = diff(&views, &views);
        assert!(patches.is_empty());
    }

    #[test]
    fn test_diff_new_view() {
        let old = vec![];
        let new = vec![View::Text {
            content: "hello".into(),
            style: Style::default(),
        }];
        let patches = diff(&old, &new);
        assert_eq!(patches.len(), 1);
        match &patches[0] {
            Patch::AppendChild { .. } => {}
            _ => panic!("expected AppendChild"),
        }
    }

    #[test]
    fn test_diff_replace_view() {
        let old = vec![View::Text {
            content: "hello".into(),
            style: Style::default(),
        }];
        let new = vec![View::Text {
            content: "world".into(),
            style: Style::default(),
        }];
        let patches = diff(&old, &new);
        assert_eq!(patches.len(), 1);
        match &patches[0] {
            Patch::Replace { .. } => {}
            _ => panic!("expected Replace"),
        }
    }
}
