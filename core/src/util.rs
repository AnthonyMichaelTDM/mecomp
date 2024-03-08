//! Utility types and functions.

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum OneOrMany<T> {
    One(T),
    Many(Arc<[T]>),
}
