//! Settings › Machines' side of the machine manager: carries the page's request, hands the page its
//! facts, and asks what the machines have free while the page shows.
use super::{Change, Manager, SurfaceSlot};
use crate::{
    client_view::ClientView, machine_picker::Choice, machine_settings::Request, machines::Store,
    terminal_device::WindowDimensions,
};
use std::time::Instant;

impl Manager {
    /// One tick of Settings › Machines, before the picker's exchanges read the outcomes it
    /// settles. `shown` is `None` while Settings is closed, otherwise whether the page shows.
    pub(super) fn exchange_settings(
        &mut self,
        shown: Option<bool>,
        surface: &mut SurfaceSlot<'_>,
        view: &mut ClientView,
        dimensions: WindowDimensions,
        now: Instant,
        before_switch: &mut dyn FnMut(&mut dyn crate::terminal_input::TerminalInputPort),
    ) -> Change {
        let mut change = Change::default();
        self.settings
            .settle(&mut self.discovery, &self.profiles, now);
        if let Some(request) = surface.active().take_machine_settings_request() {
            change.redraw = true;
            if matches!(request, Request::Add) {
                view.machines.open_add_from_settings();
            } else {
                let store = Store::configured().ok();
                let carried = self.settings.carry(
                    request,
                    &mut self.discovery,
                    store.as_ref(),
                    &self.profiles,
                    now,
                );
                if let Some(catalog) = carried.catalog {
                    let refreshed = self.refresh_catalog(
                        Ok(catalog),
                        &carried.left,
                        surface,
                        view,
                        dimensions,
                        before_switch,
                    );
                    change.switched |= refreshed.switched;
                }
                // The machine on screen was removed, disconnected or is being stopped: the view
                // returns to this computer instead of showing its terminals end.
                if self
                    .current
                    .as_ref()
                    .is_some_and(|current| carried.left.contains(&current.id))
                {
                    self.cancel();
                    self.restore(
                        &Choice::Local,
                        surface,
                        view,
                        dimensions,
                        false,
                        before_switch,
                    );
                    change.switched = true;
                }
            }
        }
        // Add Machine, opened from the page, ended: the picker closed and the page focuses the
        // machine it saved.
        if let Some(added) = view.machines.take_settings_add() {
            change.redraw = true;
            if let Some(machine) = added {
                self.settings.added(machine);
            }
        }
        if let Some(visible) = shown {
            if visible {
                self.discovery.sample_capacity(now);
            }
            let machines = &view.machines;
            let facts = self.settings.facts(
                &mut self.discovery,
                &self.profiles,
                |machine| machines.working_agents(machine),
                now,
            );
            change.redraw |= surface.active().observe_machine_settings(facts);
        }
        change
    }
}
