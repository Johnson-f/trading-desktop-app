use super::ChartWidget;
use super::candle::Timeframe;
use super::crosshair;
use super::grid;
use super::indicators::{self as ind, ParamValues};
use super::legend::{self, LegendAction};
use super::range::Range;
use super::renderer::ChartCallback;

/// Single-letter label for Timeframe — used in the compact footer row.
fn timeframe_short(tf: Timeframe) -> &'static str {
    match tf {
        Timeframe::Minute1 => "1m",
        Timeframe::Daily => "D",
        Timeframe::Weekly => "W",
        Timeframe::Monthly => "M",
    }
}

impl ChartWidget {
    pub(super) fn paint_wgpu_candles(&self, ui: &egui::Ui, chart_rect: egui::Rect) {
        let target_format = egui_wgpu::preferred_framebuffer_format(&[
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ])
        .unwrap_or(wgpu::TextureFormat::Bgra8Unorm);
        let callback = ChartCallback {
            id: self.id,
            camera: self.camera.clone(),
            data: self.data.clone(),
            target_format,
        };
        ui.painter().add(egui_wgpu::Callback::new_paint_callback(
            chart_rect, callback,
        ));
    }

    pub(super) fn paint_grid(&self, ui: &mut egui::Ui, chart_rect: egui::Rect, modal_open: bool) {
        if !modal_open && !self.input_suppressed {
            let mut camera = self.camera.lock();
            grid::handle_price_axis_drag(ui, chart_rect, &mut camera);
        }
        let camera = self.camera.lock();
        grid::paint_price_grid(ui, chart_rect, &camera, &self.data);
        grid::paint_time_grid(
            ui,
            chart_rect,
            &camera,
            &self.data,
            self.timeframe.base_scale(),
        );
    }

    /// Render the compact single-row bottom footer (Webull-style):
    ///   [Range: MAX ▼]   [Interval:  D  W  M]             [Auto]
    pub(super) fn show_footer(&mut self, ui: &mut egui::Ui, footer_rect: egui::Rect) {
        // Hairline divider above the footer row.
        ui.painter().line_segment(
            [
                egui::pos2(footer_rect.left() + 8.0, footer_rect.top()),
                egui::pos2(footer_rect.right() - 8.0, footer_rect.top()),
            ],
            egui::Stroke::new(0.5, zaned_theme::BORDER),
        );

        self.paint_footer_row(ui, footer_rect);
    }

    fn paint_footer_row(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        use eframe::egui::{Align, CornerRadius, Layout, RichText, Vec2};
        use zaned_theme::{ACCENT_TEAL, BORDER, SURFACE_HIGH, TEXT_MUTED};

        let mut new_timeframe: Option<Timeframe> = None;
        let current_tf = self.timeframe;

        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.style_mut().interaction.tooltip_delay = 0.0;

            // Restyle ComboBox visuals to match the dark theme. egui's
            // defaults render a near-white inactive/hovered background that
            // pops against the footer AND makes the selected/highlighted
            // row's text unreadable inside the popup. Force every surface
            // to use our theme tokens.
            {
                use zaned_theme::{BG, BORDER, SURFACE, TEXT_PRIMARY};
                let visuals = &mut ui.style_mut().visuals;
                // Trigger button (closed/open) — blend with the footer's BG
                // so it reads as inline chrome, not a pill on top of the bar.
                visuals.widgets.inactive.bg_fill = BG;
                visuals.widgets.inactive.weak_bg_fill = BG;
                visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
                visuals.widgets.hovered.bg_fill = SURFACE;
                visuals.widgets.hovered.weak_bg_fill = SURFACE;
                visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
                visuals.widgets.active.bg_fill = SURFACE;
                visuals.widgets.active.weak_bg_fill = SURFACE;
                visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
                visuals.widgets.open.bg_fill = SURFACE;
                visuals.widgets.open.weak_bg_fill = SURFACE;
                visuals.widgets.open.bg_stroke = egui::Stroke::NONE;
                visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
                // Popup body fill — `extreme_bg_color` controls the dropdown
                // panel's background. Default is white.
                visuals.extreme_bg_color = SURFACE;
                visuals.window_fill = SURFACE;
                visuals.panel_fill = SURFACE;
                visuals.window_stroke = egui::Stroke::new(1.0, BORDER);
                // Hovered/selected row inside the popup — `selection.bg_fill`
                // is what egui paints behind a highlighted `selectable_value`
                // row. Default is bright blue; readable text on it is hard.
                // Use a translucent teal-tinted band that keeps the white
                // text on top fully legible.
                visuals.selection.bg_fill = BG;
                visuals.selection.stroke = egui::Stroke::new(1.0, TEXT_PRIMARY);
            }

            ui.horizontal_centered(|ui| {
                ui.add_space(8.0);

                // Range dropdown
                ui.label(RichText::new("Range:").size(11.0).color(TEXT_MUTED));
                ui.add_space(4.0);

                let prev_range = self.selected_range;
                let mut selected_range_label = Some(self.selected_range.label().to_string());
                let range_items: Vec<egui_shadcn::SelectItem> = Range::ALL
                    .iter()
                    .map(|r| egui_shadcn::SelectItem::option(r.label(), r.label()))
                    .collect();
                egui_shadcn::select_with_items(
                    ui,
                    crate::shadcn_theme::theme(),
                    egui_shadcn::SelectProps::new(
                        "chart_range_dropdown",
                        &mut selected_range_label,
                    )
                    .placeholder("Range")
                    .width(60.0),
                    &range_items,
                );
                if let Some(label) = selected_range_label.as_deref() {
                    if let Some(new_range) = Range::ALL.iter().find(|r| r.label() == label) {
                        self.selected_range = *new_range;
                    }
                }

                if prev_range != self.selected_range {
                    let r = self.selected_range;
                    if let Some(n) = r.trailing_candles() {
                        let mut camera = self.camera.lock();
                        camera.fit_to_trailing(&self.data, n);
                    } else {
                        // Max or YTD — fit to all data (YTD refinement is future work)
                        let mut camera = self.camera.lock();
                        camera.fit_to_data(&self.data);
                    }
                }

                ui.add_space(12.0);

                // Interval dropdown
                ui.label(RichText::new("Interval:").size(11.0).color(TEXT_MUTED));
                ui.add_space(4.0);

                let mut selected_tf_label = Some(timeframe_short(current_tf).to_string());
                let tf_items: Vec<egui_shadcn::SelectItem> = Timeframe::ALL
                    .iter()
                    .map(|tf| {
                        egui_shadcn::SelectItem::option(timeframe_short(*tf), timeframe_short(*tf))
                    })
                    .collect();
                egui_shadcn::select_with_items(
                    ui,
                    crate::shadcn_theme::theme(),
                    egui_shadcn::SelectProps::new(
                        "chart_interval_dropdown",
                        &mut selected_tf_label,
                    )
                    .placeholder("Interval")
                    .width(50.0),
                    &tf_items,
                );
                if let Some(label) = selected_tf_label.as_deref() {
                    if let Some(new_tf) = Timeframe::ALL
                        .iter()
                        .find(|tf| timeframe_short(**tf) == label)
                    {
                        if *new_tf != current_tf {
                            new_timeframe = Some(*new_tf);
                        }
                    }
                }

                // Auto toggle — far right
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(8.0);
                    let mut camera = self.camera.lock();
                    let is_auto = camera.auto_scale_y;
                    let color = if is_auto { ACCENT_TEAL } else { TEXT_MUTED };
                    let fill = if is_auto {
                        egui::Color32::TRANSPARENT
                    } else {
                        SURFACE_HIGH
                    };
                    let stroke = if is_auto {
                        egui::Stroke::new(1.0, ACCENT_TEAL)
                    } else {
                        egui::Stroke::new(0.5, BORDER)
                    };
                    let btn = ui.add(
                        egui::Button::new(RichText::new("Auto").size(10.0).color(color))
                            .fill(fill)
                            .stroke(stroke)
                            .corner_radius(CornerRadius::same(3))
                            .min_size(Vec2::new(40.0, 18.0)),
                    );
                    if btn.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if btn.clicked() {
                        camera.auto_scale_y = !camera.auto_scale_y;
                    }
                });
            });
        });

        if let Some(tf) = new_timeframe {
            self.set_timeframe(tf);
        }
    }

    pub(super) fn paint_main_overlays(
        &mut self,
        ui: &mut egui::Ui,
        chart_rect: egui::Rect,
        cursor_idx: Option<usize>,
    ) -> Vec<(u64, LegendAction, ParamValues)> {
        {
            let camera = self.camera.lock();
            let painter = ui.painter_at(chart_rect);
            for (active, computed) in self.manager.main_overlays() {
                active.indicator().draw_main(
                    &painter,
                    chart_rect,
                    &camera,
                    &self.data,
                    computed,
                    &active.params,
                );
            }
        }

        // Ticker info row (above OHLC). Returns its height so we can offset
        // the OHLC row directly underneath.
        let ticker_anchor = egui::Pos2::new(chart_rect.left() + 8.0, chart_rect.top() + 8.0);
        let ticker_height = if let Some(sym) = self.symbol.as_deref() {
            legend::paint_ticker_info_row(
                ui,
                chart_rect,
                sym,
                None, // company name not yet plumbed
                self.timeframe.short_label(),
                ticker_anchor,
            )
        } else {
            0.0
        };

        // OHLC header row, painted below the ticker info row.
        let ohlc_anchor = egui::Pos2::new(
            chart_rect.left() + 8.0,
            chart_rect.top() + 8.0 + ticker_height + 4.0,
        );
        legend::paint_ohlc_row(ui, chart_rect, &self.data, cursor_idx, ohlc_anchor);

        // Approximate OHLC row height (monospace 12px text is ~14px tall).
        let ohlc_height: f32 = 14.0;

        // Caret toggle button — rendered between OHLC row and indicator legend.
        {
            use egui::{Align2, FontId, Pos2, Rect, Sense, Vec2};
            use zaned_theme::{HOVER_BG, ICON_HOVER, ICON_INACTIVE};

            let btn_top = chart_rect.top() + 8.0 + ticker_height + 4.0 + ohlc_height + 6.0;
            let btn_rect = Rect::from_min_size(
                Pos2::new(chart_rect.left() + 8.0, btn_top),
                Vec2::new(16.0, 16.0),
            );

            let toggle_resp = ui.interact(btn_rect, ui.id().with("legend_toggle"), Sense::click());

            if toggle_resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                ui.painter_at(chart_rect)
                    .rect_filled(btn_rect, 4.0, HOVER_BG);
            }

            let icon = if self.indicator_legend_hidden {
                egui_phosphor::regular::CARET_UP
            } else {
                egui_phosphor::regular::CARET_DOWN
            };
            let icon_color = if toggle_resp.hovered() {
                ICON_HOVER
            } else {
                ICON_INACTIVE
            };
            ui.painter_at(chart_rect).text(
                btn_rect.center(),
                Align2::CENTER_CENTER,
                icon,
                FontId::proportional(12.0),
                icon_color,
            );

            let was_clicked = toggle_resp.clicked();
            let tooltip_text = if self.indicator_legend_hidden {
                "Show Indicator Values"
            } else {
                "Hide Indicator Values"
            };
            toggle_resp.on_hover_text(tooltip_text);

            if was_clicked {
                self.indicator_legend_hidden = !self.indicator_legend_hidden;
            }
        }

        // Snapshot legend items grouped by def_id. Release the manager borrow
        // before `draw_legend_row` takes &mut ui.
        let mut groups: Vec<legend::FamilyGroup> = Vec::new();
        for (active, computed) in self.manager.main_overlays() {
            let entries =
                active
                    .indicator()
                    .legend(&self.data, computed, &active.params, cursor_idx);
            let member = legend::FamilyMember {
                instance_id: active.instance_id,
                entries,
                params: active.params.clone(),
            };
            if let Some(g) = groups.iter_mut().find(|g| g.def_id == active.def_id) {
                g.members.push(member);
            } else {
                let family_name = ind::get(active.def_id)
                    .map(|d| d.short_name)
                    .unwrap_or("")
                    .to_string();
                groups.push(legend::FamilyGroup {
                    def_id: active.def_id,
                    family_name,
                    members: vec![member],
                });
            }
        }

        // Sort each family's members by the `period` param (ascending) so
        // `EMA(10)` always renders before `EMA(20)`, regardless of the order
        // the user added them. Instances without a period param keep their
        // insertion order (stable sort).
        for group in &mut groups {
            group
                .members
                .sort_by_key(|m| legend::period_sort_key(&m.params));
        }

        let mut actions = Vec::new();
        if !self.indicator_legend_hidden {
            for (i, group) in groups.into_iter().enumerate() {
                // Flatten every member's entries into a single row. Family name
                // prefix is drawn only when there is more than one member.
                let display_name = if group.members.len() > 1 {
                    group.family_name.as_str()
                } else {
                    ""
                };
                let combined: Vec<ind::LegendEntry> = group
                    .members
                    .iter()
                    .flat_map(|m| m.entries.iter().cloned())
                    .collect();
                let first_id = group.members[0].instance_id;
                let first_params = group.members[0].params.clone();

                let action = legend::draw_legend_row(
                    ui,
                    chart_rect,
                    egui::Pos2::new(
                        chart_rect.left() + 8.0,
                        chart_rect.top() + 28.0 + ticker_height + (i as f32 * 16.0),
                    ),
                    display_name,
                    &combined,
                    &format!("main-{}", group.def_id),
                );
                match action {
                    LegendAction::None => {}
                    LegendAction::OpenSettings => {
                        // Target the first instance — settings modal edits one
                        // instance at a time.
                        actions.push((first_id, action, first_params));
                    }
                    LegendAction::Remove => {
                        // Fan out: emit one Remove per member so the caller's
                        // per-id apply loop clears the whole family.
                        for m in &group.members {
                            actions.push((m.instance_id, LegendAction::Remove, m.params.clone()));
                        }
                    }
                }
            }
        }
        actions
    }

    pub(super) fn paint_sub_panes(
        &mut self,
        ui: &mut egui::Ui,
        total_rect: egui::Rect,
        chart_rect: egui::Rect,
        pane_slots: &[(egui::Rect, egui::Rect)],
        cursor_idx: Option<usize>,
        modal_open: bool,
    ) -> Vec<(u64, LegendAction, ParamValues)> {
        let x_mapper = self.build_x_mapper(chart_rect);
        let items = self.draw_sub_pane_indicators(ui, pane_slots, x_mapper.as_ref(), cursor_idx);
        self.handle_and_paint_dividers(ui, total_rect, pane_slots, modal_open);

        let mut actions = Vec::new();
        for (i, item) in items.into_iter().enumerate() {
            let Some((_, pane_rect)) = pane_slots.get(i) else {
                continue;
            };
            let action = legend::draw_legend_row(
                ui,
                *pane_rect,
                egui::Pos2::new(pane_rect.left() + 6.0, pane_rect.top() + 4.0),
                &item.name,
                &item.entries,
                &format!("pane-{}", item.instance_id),
            );
            if action != LegendAction::None {
                actions.push((item.instance_id, action, item.params));
            }
        }
        actions
    }

    pub(super) fn paint_crosshair(
        &self,
        ui: &egui::Ui,
        chart_rect: egui::Rect,
        full_rect: egui::Rect,
    ) {
        let camera = self.camera.lock();
        crosshair::paint_crosshair(ui, chart_rect, full_rect, &camera, &self.data);
    }

    pub(super) fn paint_notification(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        const NOTIFICATION_DURATION_SECS: f32 = 2.0;

        if let Some((message, start_time)) = &self.notification {
            let elapsed = start_time.elapsed().as_secs_f32();

            if elapsed > NOTIFICATION_DURATION_SECS {
                self.notification = None;
                return;
            }

            // Fade out in the last 0.5 seconds
            let alpha = if elapsed > NOTIFICATION_DURATION_SECS - 0.5 {
                ((NOTIFICATION_DURATION_SECS - elapsed) / 0.5).clamp(0.0, 1.0)
            } else {
                1.0
            };

            let painter = ui.painter_at(rect);
            let center = rect.center();
            let text_pos = egui::Pos2::new(center.x, rect.top() + 60.0);

            // Determine color based on message type
            let (bg_color, text_color) = if message.starts_with('✓') {
                (
                    egui::Color32::from_rgba_unmultiplied(78, 205, 196, (180.0 * alpha) as u8),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8),
                )
            } else {
                (
                    egui::Color32::from_rgba_unmultiplied(255, 107, 107, (180.0 * alpha) as u8),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, (255.0 * alpha) as u8),
                )
            };

            // Measure text size
            let font_id = egui::FontId::proportional(14.0);
            let galley = painter.layout_no_wrap(message.clone(), font_id.clone(), text_color);

            // Draw background
            let padding = egui::Vec2::new(12.0, 8.0);
            let bg_rect = egui::Rect::from_center_size(text_pos, galley.size() + padding * 2.0);
            painter.rect_filled(bg_rect, egui::CornerRadius::same(6), bg_color);

            // Draw text
            painter.galley(
                egui::Pos2::new(
                    text_pos.x - galley.size().x / 2.0,
                    text_pos.y - galley.size().y / 2.0,
                ),
                galley,
                text_color,
            );

            // Request repaint for animation
            ui.ctx().request_repaint();
        }
    }
}
