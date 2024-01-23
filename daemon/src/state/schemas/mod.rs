use std::sync::Arc;

pub mod album;
pub mod artist;
pub mod song;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum OneOrMany<T> {
    One(T),
    Many(Arc<[T]>),
}
