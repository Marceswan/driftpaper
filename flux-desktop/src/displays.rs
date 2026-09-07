use std::collections::HashSet;

/// Native identity is independent of display order and desktop coordinates.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DisplayInfo {
    pub(crate) id: String,
    pub(crate) origin_x: f64,
    pub(crate) origin_y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) pixels_wide: u32,
    pub(crate) pixels_high: u32,
}

pub(crate) fn removed_displays<'a>(
    current: &'a [DisplayInfo],
    next: &[DisplayInfo],
) -> Vec<&'a str> {
    let ids: HashSet<_> = next.iter().map(|display| display.id.as_str()).collect();
    current
        .iter()
        .filter(|display| !ids.contains(display.id.as_str()))
        .map(|display| display.id.as_str())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn display(id: &str) -> DisplayInfo {
        DisplayInfo {
            id: id.into(),
            origin_x: 0.0,
            origin_y: 0.0,
            width: 1920.0,
            height: 1080.0,
            pixels_wide: 1920,
            pixels_high: 1080,
        }
    }
    #[test]
    fn reorder_does_not_remove_existing_displays() {
        let current = [display("left"), display("right")];
        assert!(removed_displays(&current, &[display("right"), display("left")]).is_empty());
    }
    #[test]
    fn unplug_and_replacement_use_identity_not_count() {
        let current = [display("left"), display("right")];
        assert_eq!(
            removed_displays(&current, &[display("replacement"), display("right")]),
            vec!["left"]
        );
        assert_eq!(removed_displays(&current, &[]), vec!["left", "right"]);
    }
}
