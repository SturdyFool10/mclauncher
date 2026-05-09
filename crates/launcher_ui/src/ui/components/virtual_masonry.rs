use egui::{Rect, Ui};
use ui_foundation::responsive_columns;

const MASONRY_WIDTH_BUCKET_POINTS: f32 = 32.0;

#[derive(Clone, Debug)]
pub struct VirtualMasonryItem {
    pub item_index: usize,
    pub offset_y: f32,
    pub height: f32,
}

#[derive(Clone, Debug)]
pub struct VirtualMasonryColumn {
    pub items: Vec<VirtualMasonryItem>,
    pub total_height: f32,
}

#[derive(Clone, Debug)]
pub struct VirtualMasonryLayout {
    pub columns: Vec<VirtualMasonryColumn>,
    pub content_width: f32,
    pub column_width: f32,
}

#[derive(Clone, Debug)]
pub struct CachedVirtualMasonryLayout {
    pub width_bucket: u32,
    pub revision: u64,
    pub layout: VirtualMasonryLayout,
}

pub fn build_virtual_masonry_layout(
    ui: &Ui,
    min_column_width: f32,
    gap: f32,
    max_columns: usize,
    item_count: usize,
    revision: u64,
    cache: &mut Option<CachedVirtualMasonryLayout>,
    mut item_height: impl FnMut(usize, f32) -> f32,
) -> VirtualMasonryLayout {
    let content_width = ui.available_width().max(min_column_width);
    let (column_count, column_width) =
        responsive_columns(content_width, min_column_width, gap, max_columns);
    let width_bucket = (column_width / MASONRY_WIDTH_BUCKET_POINTS)
        .round()
        .max(1.0) as u32;
    if let Some(cached) = cache.as_ref()
        && cached.width_bucket == width_bucket
        && cached.revision == revision
    {
        return cached.layout.clone();
    }

    let mut columns = (0..column_count)
        .map(|_| VirtualMasonryColumn {
            items: Vec::new(),
            total_height: 0.0,
        })
        .collect::<Vec<_>>();

    for item_index in 0..item_count {
        let height = item_height(item_index, column_width);
        let target_column = columns
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.total_height.total_cmp(&right.total_height))
            .map(|(index, _)| index)
            .unwrap_or(0);
        let offset_y = columns[target_column].total_height;
        columns[target_column].items.push(VirtualMasonryItem {
            item_index,
            offset_y,
            height,
        });
        columns[target_column].total_height += height + gap;
    }

    let layout = VirtualMasonryLayout {
        columns,
        content_width,
        column_width,
    };
    *cache = Some(CachedVirtualMasonryLayout {
        width_bucket,
        revision,
        layout: layout.clone(),
    });
    layout
}

pub fn render_virtualized_masonry(
    ui: &mut Ui,
    layout: &VirtualMasonryLayout,
    gap: f32,
    viewport: Rect,
    overscan: f32,
    mut render_item: impl FnMut(&mut Ui, usize, f32),
) {
    let visible_top = viewport.top() - overscan;
    let visible_bottom = viewport.bottom() + overscan;
    ui.allocate_ui_with_layout(
        egui::vec2(layout.content_width, 0.0),
        egui::Layout::left_to_right(egui::Align::Min),
        |ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for column in &layout.columns {
                ui.allocate_ui_with_layout(
                    egui::vec2(layout.column_width, 0.0),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(layout.column_width);
                        let mut visible_started = false;
                        let mut skipped_height = 0.0;
                        let mut rendered_bottom = 0.0;

                        for item in &column.items {
                            let item_bottom = item.offset_y + item.height;
                            if item_bottom < visible_top {
                                skipped_height = item_bottom + gap;
                                continue;
                            }
                            if item.offset_y > visible_bottom {
                                break;
                            }
                            if !visible_started {
                                if skipped_height > 0.0 {
                                    ui.add_space(skipped_height);
                                }
                                visible_started = true;
                            }
                            render_item(ui, item.item_index, item.height);
                            rendered_bottom = item_bottom + gap;
                            ui.add_space(gap);
                        }

                        if !visible_started {
                            if column.total_height > 0.0 {
                                ui.add_space(column.total_height);
                            }
                        } else {
                            let trailing_height = (column.total_height - rendered_bottom).max(0.0);
                            if trailing_height > 0.0 {
                                ui.add_space(trailing_height);
                            }
                        }
                    },
                );
            }
        },
    );
}
