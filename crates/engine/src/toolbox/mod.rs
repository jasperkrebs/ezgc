//! The toolbox: reusable steps that project kinds are built from.
//!
//! A kind composes these and stays small; a technique one EasyGCode project invented (a ring fill,
//! a brim, a pattern) goes here, written once, so every kind can use it. Each step adds moves to a
//! [`crate::toolpath::Toolpath`] and never writes G-code.

pub mod shapes;
pub mod vase;
