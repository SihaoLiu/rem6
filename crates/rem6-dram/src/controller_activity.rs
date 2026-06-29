use std::collections::BTreeMap;

use crate::activity::{
    collect_dram_bank_activity, collect_dram_bank_activity_until, collect_dram_port_activity,
    collect_dram_port_activity_until, record_future_terminal_memory_activity,
};
use crate::refresh::{record_due_refresh_events, DramRefreshWindow};
use crate::{
    low_power, record_low_power_before_refreshes, DramAccess, DramActivityMarker,
    DramActivityProfile, DramBankActivity, DramController, DramPortActivity,
};

impl DramController {
    pub fn mark_activity(&self) -> DramActivityMarker {
        DramActivityMarker::new(self.activity_log.len())
    }

    fn activity_log_since(&self, marker: DramActivityMarker) -> &[DramAccess] {
        self.activity_log.get(marker.offset..).unwrap_or(&[])
    }

    pub fn bank_activities(&self) -> BTreeMap<(u32, u32), DramBankActivity> {
        collect_dram_bank_activity(&self.activity_log)
    }

    pub fn bank_activities_since(
        &self,
        marker: DramActivityMarker,
    ) -> BTreeMap<(u32, u32), DramBankActivity> {
        collect_dram_bank_activity(self.activity_log_since(marker))
    }

    fn bank_activities_in(
        &self,
        accesses: &[DramAccess],
        end_cycle: u64,
    ) -> BTreeMap<(u32, u32), DramBankActivity> {
        let mut activities = collect_dram_bank_activity_until(accesses, end_cycle);
        self.record_terminal_memory_activity(&mut activities, end_cycle);
        record_future_terminal_memory_activity(accesses, &mut activities, end_cycle);
        activities
    }

    pub fn bank_activities_until(&self, end_cycle: u64) -> BTreeMap<(u32, u32), DramBankActivity> {
        self.bank_activities_in(&self.activity_log, end_cycle)
    }

    pub fn bank_activities_since_until(
        &self,
        marker: DramActivityMarker,
        end_cycle: u64,
    ) -> BTreeMap<(u32, u32), DramBankActivity> {
        self.bank_activities_in(self.activity_log_since(marker), end_cycle)
    }

    pub fn bank_activity(&self, parallel_port: u32, bank: u32) -> Option<DramBankActivity> {
        self.bank_activities().remove(&(parallel_port, bank))
    }

    pub fn port_activities(&self) -> BTreeMap<u32, DramPortActivity> {
        collect_dram_port_activity(&self.activity_log)
    }

    pub fn port_activities_since(
        &self,
        marker: DramActivityMarker,
    ) -> BTreeMap<u32, DramPortActivity> {
        collect_dram_port_activity(self.activity_log_since(marker))
    }

    pub fn port_activities_until(&self, end_cycle: u64) -> BTreeMap<u32, DramPortActivity> {
        collect_dram_port_activity_until(&self.activity_log, end_cycle)
    }

    pub fn port_activities_since_until(
        &self,
        marker: DramActivityMarker,
        end_cycle: u64,
    ) -> BTreeMap<u32, DramPortActivity> {
        collect_dram_port_activity_until(self.activity_log_since(marker), end_cycle)
    }

    pub fn port_activity(&self, parallel_port: u32) -> Option<DramPortActivity> {
        self.port_activities().remove(&parallel_port)
    }

    pub fn activity_profile(&self) -> DramActivityProfile {
        DramActivityProfile::from_activities(&self.port_activities(), &self.bank_activities())
    }

    pub fn activity_profile_until(&self, end_cycle: u64) -> DramActivityProfile {
        DramActivityProfile::from_activities(
            &self.port_activities_until(end_cycle),
            &self.bank_activities_until(end_cycle),
        )
    }

    pub fn activity_profile_since(&self, marker: DramActivityMarker) -> DramActivityProfile {
        DramActivityProfile::from_activities(
            &self.port_activities_since(marker),
            &self.bank_activities_since(marker),
        )
    }

    pub fn activity_profile_since_until(
        &self,
        marker: DramActivityMarker,
        end_cycle: u64,
    ) -> DramActivityProfile {
        DramActivityProfile::from_activities(
            &self.port_activities_since_until(marker, end_cycle),
            &self.bank_activities_since_until(marker, end_cycle),
        )
    }

    fn record_terminal_memory_activity(
        &self,
        bank_activities: &mut BTreeMap<(u32, u32), DramBankActivity>,
        end_cycle: u64,
    ) {
        if end_cycle == 0 {
            return;
        }
        let bank_count = self.geometry.bank_count() as usize;
        let terminal_banks =
            if self.timing.refresh_timing().is_some() || self.timing.low_power_timing().is_some() {
                (0..self.banks.len())
                    .map(|bank_index| {
                        (
                            (bank_index / bank_count) as u32,
                            (bank_index % bank_count) as u32,
                        )
                    })
                    .collect::<Vec<_>>()
            } else {
                let all_bank_activities = self.bank_activities();
                all_bank_activities.keys().copied().collect::<Vec<_>>()
            };
        for (parallel_port, local_bank) in terminal_banks {
            let bank_index = parallel_port as usize * bank_count + local_bank as usize;
            let Some(bank) = self.banks.get(bank_index) else {
                continue;
            };
            let Some(port) = self.ports.get(parallel_port as usize) else {
                continue;
            };
            let mut bank = *bank;
            let idle_start_cycle = port.bus_available_cycle().max(bank.available_cycle());
            let has_open_row = bank.open_row().is_some();
            let mut waits = Vec::new();
            let refresh_events = if let Some(refresh_timing) = self.timing.refresh_timing() {
                record_due_refresh_events(
                    refresh_timing,
                    &mut bank,
                    DramRefreshWindow::maintenance(
                        parallel_port,
                        local_bank,
                        end_cycle.saturating_sub(1),
                    ),
                    &mut waits,
                )
            } else {
                Vec::new()
            };
            let mut low_power_events = Vec::new();
            if let Some(low_power_timing) = self.timing.low_power_timing() {
                record_low_power_before_refreshes(
                    low_power_timing,
                    parallel_port,
                    idle_start_cycle,
                    has_open_row,
                    &refresh_events,
                    &mut low_power_events,
                );
                low_power_events.extend(low_power::events_for_idle_window(
                    low_power_timing,
                    parallel_port,
                    port.bus_available_cycle().max(bank.available_cycle()),
                    end_cycle,
                    bank.open_row().is_some(),
                ));
            }
            if !refresh_events.is_empty() || !low_power_events.is_empty() {
                let activity = bank_activities
                    .entry((parallel_port, local_bank))
                    .or_default();
                activity.record_terminal_refresh_events(&refresh_events, end_cycle);
                activity.record_terminal_low_power_events(&low_power_events);
            }
        }
    }

    pub fn clear_activity(&mut self) {
        self.activity_log.clear();
    }
}
