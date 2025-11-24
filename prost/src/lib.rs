// we can't really control the code-gen, so we have to allow some lints here
#![allow(
    clippy::derive_partial_eq_without_eq,
    clippy::missing_const_for_fn,
    clippy::too_many_lines,
    clippy::default_trait_access,
    clippy::doc_markdown,
    clippy::missing_errors_doc,
    clippy::must_use_candidate
)]

mod mecomp {
    include!("../out/mecomp.rs");
}

pub use mecomp::*;
