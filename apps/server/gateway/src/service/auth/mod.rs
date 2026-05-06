//! Auth subsystem: Clerk JWT verification.
//!
//! The host process boots the auth layer via `runtime::build_layer` and
//! applies it to whichever routes need protection (currently `/ws`; future
//! REST endpoints when they land). `/health` remains public.

pub mod runtime;
