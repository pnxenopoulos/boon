//! CLI command implementations.

mod classes;
mod entities;
mod events;
mod info;
mod messages;
mod send_tables;
mod string_tables;
mod verify;

pub use classes::run as classes;
pub use entities::run as entities;
pub use events::run as events;
pub use info::run as info;
pub use messages::run as messages;
pub use send_tables::run as send_tables;
pub use string_tables::run as string_tables;
pub use verify::run as verify;
