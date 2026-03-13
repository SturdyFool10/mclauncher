use crate::screens::AppScreen;

#[derive(Debug, Clone, Default)]
pub struct InstanceScreenOutput {
    pub instances_changed: bool,
    pub requested_screen: Option<AppScreen>,
}
