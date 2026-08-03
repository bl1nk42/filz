pub mod behavior;
pub mod cache;
pub mod models;
pub mod reporting;

pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/_.rs"));
}
