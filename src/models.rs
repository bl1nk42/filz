use crate::proto as pb;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
    pub type_: String,
    pub stack: String,
    pub category_path: String,
    pub path: String,
    pub created_at: String,
}

#[derive(Debug, Default, Clone)]
pub struct Registry {
    pub items: HashMap<String, Item>,
}

impl Registry {
    pub fn to_proto(&self) -> pb::Registry {
        pb::Registry {
            items: self
                .items
                .values()
                .map(|it| pb::Item {
                    name: it.name.clone(),
                    r#type: it.type_.clone(),
                    stack: it.stack.clone(),
                    category_path: it.category_path.clone(),
                    path: it.path.clone(),
                    created_at: it.created_at.clone(),
                })
                .collect(),
        }
    }

    pub fn from_proto(pb_registry: pb::Registry) -> Self {
        let mut items = HashMap::new();
        for it in pb_registry.items {
            items.insert(
                it.name.clone(),
                Item {
                    name: it.name,
                    type_: it.r#type,
                    stack: it.stack,
                    category_path: it.category_path,
                    path: it.path,
                    created_at: it.created_at,
                },
            );
        }
        Registry { items }
    }
}

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Message(String),
}

pub type AppResult<T> = Result<T, AppError>;

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Json(e)
    }
}

impl From<dialoguer::Error> for AppError {
    fn from(e: dialoguer::Error) -> Self {
        AppError::Message(e.to_string())
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "{e}"),
            AppError::Json(e) => write!(f, "{e}"),
            AppError::Message(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AppError {}
