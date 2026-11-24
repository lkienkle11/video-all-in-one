//! Application services that turn domain values into plans.

pub mod job_scheduler;
pub mod media_planner;

#[cfg(test)]
mod job_scheduler_fixtures;
#[cfg(test)]
mod job_scheduler_tests;

// These tests spell paths the Unix way (`/out/clip-01.mp4`), which Windows does not
// treat as absolute; `media_planner_windows_tests` covers the same rules with drive letters.
#[cfg(all(test, unix))]
mod media_planner_fixtures;
#[cfg(all(test, unix))]
mod media_planner_guard_tests;
#[cfg(all(test, unix))]
mod media_planner_tests;
#[cfg(all(test, windows))]
mod media_planner_windows_tests;
