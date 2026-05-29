use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, SchedulerContext, Tick};

use crate::{
    IdeChannelId, IdeCommandIssue, IdeController, IdeControllerError, IdeControllerGuestMemory,
    IdeDataReadIssue, IdeDataWriteIssue, IdeDeviceId, IdeDiskError, IdeDiskTransferSnapshot,
    IdePciInterruptPort, IDE_BMI_COMMAND_START,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdeTimedDataRead {
    word: u16,
    completion_event: Option<PartitionEventId>,
}

impl IdeTimedDataRead {
    const fn new(word: u16, completion_event: Option<PartitionEventId>) -> Self {
        Self {
            word,
            completion_event,
        }
    }

    pub const fn word(self) -> u16 {
        self.word
    }

    pub const fn completion_event(self) -> Option<PartitionEventId> {
        self.completion_event
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdeTimedDataWrite {
    completion_event: Option<PartitionEventId>,
}

impl IdeTimedDataWrite {
    const fn new(completion_event: Option<PartitionEventId>) -> Self {
        Self { completion_event }
    }

    pub const fn completion_event(self) -> Option<PartitionEventId> {
        self.completion_event
    }
}

#[derive(Clone, Debug)]
pub struct IdeControllerTimingPort {
    controller: Arc<Mutex<IdeController>>,
    delay_ticks: Tick,
    interrupt_port: Option<IdePciInterruptPort>,
    completion_errors: Arc<Mutex<Vec<IdeControllerError>>>,
}

impl IdeControllerTimingPort {
    pub fn new(
        controller: Arc<Mutex<IdeController>>,
        delay_ticks: Tick,
    ) -> Result<Self, IdeControllerError> {
        if delay_ticks == 0 {
            return Err(IdeControllerError::ZeroTimingDelay);
        }
        Ok(Self {
            controller,
            delay_ticks,
            interrupt_port: None,
            completion_errors: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn with_interrupt_port(mut self, interrupt_port: IdePciInterruptPort) -> Self {
        self.interrupt_port = Some(interrupt_port);
        self
    }

    pub fn controller(&self) -> Arc<Mutex<IdeController>> {
        Arc::clone(&self.controller)
    }

    pub const fn delay_ticks(&self) -> Tick {
        self.delay_ticks
    }

    pub fn completion_errors(&self) -> Arc<Mutex<Vec<IdeControllerError>>> {
        Arc::clone(&self.completion_errors)
    }

    pub fn write_command_u8(
        &self,
        context: &mut SchedulerContext<'_>,
        channel: IdeChannelId,
        offset: u8,
        value: u8,
    ) -> Result<Option<PartitionEventId>, IdeControllerError> {
        let issue = self
            .controller
            .lock()
            .expect("IDE controller timing lock")
            .write_command_u8_timed(channel, offset, value)?;
        self.schedule_if_delayed(context, channel, issue)
    }

    pub fn write_command_u8_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        channel: IdeChannelId,
        offset: u8,
        value: u8,
    ) -> Result<Option<PartitionEventId>, IdeControllerError> {
        let issue = self
            .controller
            .lock()
            .expect("IDE controller timing lock")
            .write_command_u8_timed(channel, offset, value)?;
        self.schedule_if_delayed_parallel(context, channel, issue)
    }

    pub fn read_data_u16(
        &self,
        context: &mut SchedulerContext<'_>,
        channel: IdeChannelId,
    ) -> Result<IdeTimedDataRead, IdeControllerError> {
        let issue = self
            .controller
            .lock()
            .expect("IDE controller timing lock")
            .read_data_u16_timed(channel)?;
        self.schedule_read_if_delayed(context, channel, issue)
    }

    pub fn read_data_u16_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        channel: IdeChannelId,
    ) -> Result<IdeTimedDataRead, IdeControllerError> {
        let issue = self
            .controller
            .lock()
            .expect("IDE controller timing lock")
            .read_data_u16_timed(channel)?;
        self.schedule_read_if_delayed_parallel(context, channel, issue)
    }

    pub fn write_data_u16(
        &self,
        context: &mut SchedulerContext<'_>,
        channel: IdeChannelId,
        value: u16,
    ) -> Result<IdeTimedDataWrite, IdeControllerError> {
        let issue = self
            .controller
            .lock()
            .expect("IDE controller timing lock")
            .write_data_u16_timed(channel, value)?;
        self.schedule_write_if_delayed(context, channel, issue)
    }

    pub fn write_data_u16_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        channel: IdeChannelId,
        value: u16,
    ) -> Result<IdeTimedDataWrite, IdeControllerError> {
        let issue = self
            .controller
            .lock()
            .expect("IDE controller timing lock")
            .write_data_u16_timed(channel, value)?;
        self.schedule_write_if_delayed_parallel(context, channel, issue)
    }

    pub fn execute_dma<G>(
        &self,
        context: &mut SchedulerContext<'_>,
        channel: IdeChannelId,
        guest: Arc<Mutex<G>>,
    ) -> Result<Option<PartitionEventId>, IdeControllerError>
    where
        G: IdeControllerGuestMemory + Send + 'static,
    {
        let delay = dma_delay_ticks(&self.controller, channel, self.delay_ticks)?;
        let controller = Arc::clone(&self.controller);
        let interrupt_port = self.interrupt_port.clone();
        let completion_errors = Arc::clone(&self.completion_errors);
        context
            .schedule_local_after(delay, move |context| {
                complete_dma(
                    &controller,
                    interrupt_port.as_ref(),
                    &guest,
                    &completion_errors,
                    channel,
                    context,
                );
            })
            .map(Some)
            .map_err(scheduler_error)
    }

    pub fn execute_dma_parallel<G>(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        channel: IdeChannelId,
        guest: Arc<Mutex<G>>,
    ) -> Result<Option<PartitionEventId>, IdeControllerError>
    where
        G: IdeControllerGuestMemory + Send + 'static,
    {
        let delay = dma_delay_ticks(&self.controller, channel, self.delay_ticks)?;
        let controller = Arc::clone(&self.controller);
        let interrupt_port = self.interrupt_port.clone();
        let completion_errors = Arc::clone(&self.completion_errors);
        context
            .schedule_local_after(delay, move |context| {
                complete_dma_parallel(
                    &controller,
                    interrupt_port.as_ref(),
                    &guest,
                    &completion_errors,
                    channel,
                    context,
                );
            })
            .map(Some)
            .map_err(scheduler_error)
    }

    fn schedule_if_delayed(
        &self,
        context: &mut SchedulerContext<'_>,
        channel: IdeChannelId,
        issue: IdeCommandIssue,
    ) -> Result<Option<PartitionEventId>, IdeControllerError> {
        if issue == IdeCommandIssue::Completed {
            return self.sync_interrupt(context);
        }

        let controller = Arc::clone(&self.controller);
        let interrupt_port = self.interrupt_port.clone();
        let completion_errors = Arc::clone(&self.completion_errors);
        context
            .schedule_local_after(self.delay_ticks, move |context| {
                complete_timed_command(
                    &controller,
                    interrupt_port.as_ref(),
                    &completion_errors,
                    channel,
                    context,
                );
            })
            .map(Some)
            .map_err(scheduler_error)
    }

    fn schedule_if_delayed_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        channel: IdeChannelId,
        issue: IdeCommandIssue,
    ) -> Result<Option<PartitionEventId>, IdeControllerError> {
        if issue == IdeCommandIssue::Completed {
            return self.sync_interrupt_parallel(context);
        }

        let controller = Arc::clone(&self.controller);
        let interrupt_port = self.interrupt_port.clone();
        let completion_errors = Arc::clone(&self.completion_errors);
        context
            .schedule_local_after(self.delay_ticks, move |context| {
                complete_timed_command_parallel(
                    &controller,
                    interrupt_port.as_ref(),
                    &completion_errors,
                    channel,
                    context,
                );
            })
            .map(Some)
            .map_err(scheduler_error)
    }

    fn schedule_read_if_delayed(
        &self,
        context: &mut SchedulerContext<'_>,
        channel: IdeChannelId,
        issue: IdeDataReadIssue,
    ) -> Result<IdeTimedDataRead, IdeControllerError> {
        if !issue.sector_delay() {
            self.sync_interrupt(context)?;
            return Ok(IdeTimedDataRead::new(issue.word(), None));
        }

        let controller = Arc::clone(&self.controller);
        let interrupt_port = self.interrupt_port.clone();
        let completion_errors = Arc::clone(&self.completion_errors);
        let completion_event = context
            .schedule_local_after(self.delay_ticks, move |context| {
                complete_timed_data_read(
                    &controller,
                    interrupt_port.as_ref(),
                    &completion_errors,
                    channel,
                    context,
                );
            })
            .map_err(scheduler_error)?;
        self.sync_interrupt(context)?;
        Ok(IdeTimedDataRead::new(issue.word(), Some(completion_event)))
    }

    fn schedule_read_if_delayed_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        channel: IdeChannelId,
        issue: IdeDataReadIssue,
    ) -> Result<IdeTimedDataRead, IdeControllerError> {
        if !issue.sector_delay() {
            self.sync_interrupt_parallel(context)?;
            return Ok(IdeTimedDataRead::new(issue.word(), None));
        }

        let controller = Arc::clone(&self.controller);
        let interrupt_port = self.interrupt_port.clone();
        let completion_errors = Arc::clone(&self.completion_errors);
        let completion_event = context
            .schedule_local_after(self.delay_ticks, move |context| {
                complete_timed_data_read_parallel(
                    &controller,
                    interrupt_port.as_ref(),
                    &completion_errors,
                    channel,
                    context,
                );
            })
            .map_err(scheduler_error)?;
        self.sync_interrupt_parallel(context)?;
        Ok(IdeTimedDataRead::new(issue.word(), Some(completion_event)))
    }

    fn schedule_write_if_delayed(
        &self,
        context: &mut SchedulerContext<'_>,
        channel: IdeChannelId,
        issue: IdeDataWriteIssue,
    ) -> Result<IdeTimedDataWrite, IdeControllerError> {
        if !issue.sector_delay() {
            self.sync_interrupt(context)?;
            return Ok(IdeTimedDataWrite::new(None));
        }

        let controller = Arc::clone(&self.controller);
        let interrupt_port = self.interrupt_port.clone();
        let completion_errors = Arc::clone(&self.completion_errors);
        let completion_event = context
            .schedule_local_after(self.delay_ticks, move |context| {
                complete_timed_data_write(
                    &controller,
                    interrupt_port.as_ref(),
                    &completion_errors,
                    channel,
                    context,
                );
            })
            .map_err(scheduler_error)?;
        self.sync_interrupt(context)?;
        Ok(IdeTimedDataWrite::new(Some(completion_event)))
    }

    fn schedule_write_if_delayed_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        channel: IdeChannelId,
        issue: IdeDataWriteIssue,
    ) -> Result<IdeTimedDataWrite, IdeControllerError> {
        if !issue.sector_delay() {
            self.sync_interrupt_parallel(context)?;
            return Ok(IdeTimedDataWrite::new(None));
        }

        let controller = Arc::clone(&self.controller);
        let interrupt_port = self.interrupt_port.clone();
        let completion_errors = Arc::clone(&self.completion_errors);
        let completion_event = context
            .schedule_local_after(self.delay_ticks, move |context| {
                complete_timed_data_write_parallel(
                    &controller,
                    interrupt_port.as_ref(),
                    &completion_errors,
                    channel,
                    context,
                );
            })
            .map_err(scheduler_error)?;
        self.sync_interrupt_parallel(context)?;
        Ok(IdeTimedDataWrite::new(Some(completion_event)))
    }

    fn sync_interrupt(
        &self,
        context: &mut SchedulerContext<'_>,
    ) -> Result<Option<PartitionEventId>, IdeControllerError> {
        if let Some(interrupt_port) = &self.interrupt_port {
            let controller = self.controller.lock().expect("IDE controller timing lock");
            interrupt_port.sync_controller(context, &controller)
        } else {
            Ok(None)
        }
    }

    fn sync_interrupt_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
    ) -> Result<Option<PartitionEventId>, IdeControllerError> {
        if let Some(interrupt_port) = &self.interrupt_port {
            let controller = self.controller.lock().expect("IDE controller timing lock");
            interrupt_port.sync_controller_parallel(context, &controller)
        } else {
            Ok(None)
        }
    }
}

fn complete_timed_command(
    controller: &Arc<Mutex<IdeController>>,
    interrupt_port: Option<&IdePciInterruptPort>,
    completion_errors: &Arc<Mutex<Vec<IdeControllerError>>>,
    channel: IdeChannelId,
    context: &mut SchedulerContext<'_>,
) {
    let mut controller = controller.lock().expect("IDE controller timing lock");
    if let Err(error) = controller.complete_timed_command(channel) {
        completion_errors
            .lock()
            .expect("IDE timing completion error lock")
            .push(error);
        return;
    }
    if let Some(interrupt_port) = interrupt_port {
        if let Err(error) = interrupt_port.sync_controller(context, &controller) {
            completion_errors
                .lock()
                .expect("IDE timing completion error lock")
                .push(error);
        }
    }
}

fn complete_timed_command_parallel(
    controller: &Arc<Mutex<IdeController>>,
    interrupt_port: Option<&IdePciInterruptPort>,
    completion_errors: &Arc<Mutex<Vec<IdeControllerError>>>,
    channel: IdeChannelId,
    context: &mut ParallelSchedulerContext<'_>,
) {
    let mut controller = controller.lock().expect("IDE controller timing lock");
    if let Err(error) = controller.complete_timed_command(channel) {
        completion_errors
            .lock()
            .expect("IDE timing completion error lock")
            .push(error);
        return;
    }
    if let Some(interrupt_port) = interrupt_port {
        if let Err(error) = interrupt_port.sync_controller_parallel(context, &controller) {
            completion_errors
                .lock()
                .expect("IDE timing completion error lock")
                .push(error);
        }
    }
}

fn complete_timed_data_read(
    controller: &Arc<Mutex<IdeController>>,
    interrupt_port: Option<&IdePciInterruptPort>,
    completion_errors: &Arc<Mutex<Vec<IdeControllerError>>>,
    channel: IdeChannelId,
    context: &mut SchedulerContext<'_>,
) {
    let mut controller = controller.lock().expect("IDE controller timing lock");
    if let Err(error) = controller.complete_timed_data_read(channel) {
        completion_errors
            .lock()
            .expect("IDE timing completion error lock")
            .push(error);
        return;
    }
    if let Some(interrupt_port) = interrupt_port {
        if let Err(error) = interrupt_port.sync_controller(context, &controller) {
            completion_errors
                .lock()
                .expect("IDE timing completion error lock")
                .push(error);
        }
    }
}

fn complete_timed_data_read_parallel(
    controller: &Arc<Mutex<IdeController>>,
    interrupt_port: Option<&IdePciInterruptPort>,
    completion_errors: &Arc<Mutex<Vec<IdeControllerError>>>,
    channel: IdeChannelId,
    context: &mut ParallelSchedulerContext<'_>,
) {
    let mut controller = controller.lock().expect("IDE controller timing lock");
    if let Err(error) = controller.complete_timed_data_read(channel) {
        completion_errors
            .lock()
            .expect("IDE timing completion error lock")
            .push(error);
        return;
    }
    if let Some(interrupt_port) = interrupt_port {
        if let Err(error) = interrupt_port.sync_controller_parallel(context, &controller) {
            completion_errors
                .lock()
                .expect("IDE timing completion error lock")
                .push(error);
        }
    }
}

fn complete_timed_data_write(
    controller: &Arc<Mutex<IdeController>>,
    interrupt_port: Option<&IdePciInterruptPort>,
    completion_errors: &Arc<Mutex<Vec<IdeControllerError>>>,
    channel: IdeChannelId,
    context: &mut SchedulerContext<'_>,
) {
    let mut controller = controller.lock().expect("IDE controller timing lock");
    if let Err(error) = controller.complete_timed_data_write(channel) {
        completion_errors
            .lock()
            .expect("IDE timing completion error lock")
            .push(error);
        return;
    }
    if let Some(interrupt_port) = interrupt_port {
        if let Err(error) = interrupt_port.sync_controller(context, &controller) {
            completion_errors
                .lock()
                .expect("IDE timing completion error lock")
                .push(error);
        }
    }
}

fn complete_timed_data_write_parallel(
    controller: &Arc<Mutex<IdeController>>,
    interrupt_port: Option<&IdePciInterruptPort>,
    completion_errors: &Arc<Mutex<Vec<IdeControllerError>>>,
    channel: IdeChannelId,
    context: &mut ParallelSchedulerContext<'_>,
) {
    let mut controller = controller.lock().expect("IDE controller timing lock");
    if let Err(error) = controller.complete_timed_data_write(channel) {
        completion_errors
            .lock()
            .expect("IDE timing completion error lock")
            .push(error);
        return;
    }
    if let Some(interrupt_port) = interrupt_port {
        if let Err(error) = interrupt_port.sync_controller_parallel(context, &controller) {
            completion_errors
                .lock()
                .expect("IDE timing completion error lock")
                .push(error);
        }
    }
}

fn complete_dma<G>(
    controller: &Arc<Mutex<IdeController>>,
    interrupt_port: Option<&IdePciInterruptPort>,
    guest: &Arc<Mutex<G>>,
    completion_errors: &Arc<Mutex<Vec<IdeControllerError>>>,
    channel: IdeChannelId,
    context: &mut SchedulerContext<'_>,
) where
    G: IdeControllerGuestMemory,
{
    let mut controller = controller.lock().expect("IDE controller timing lock");
    {
        let mut guest = guest.lock().expect("IDE DMA guest memory lock");
        if let Err(error) = controller.execute_dma(channel, &mut *guest) {
            completion_errors
                .lock()
                .expect("IDE timing completion error lock")
                .push(error);
            return;
        }
    }
    if let Some(interrupt_port) = interrupt_port {
        if let Err(error) = interrupt_port.sync_controller(context, &controller) {
            completion_errors
                .lock()
                .expect("IDE timing completion error lock")
                .push(error);
        }
    }
}

fn complete_dma_parallel<G>(
    controller: &Arc<Mutex<IdeController>>,
    interrupt_port: Option<&IdePciInterruptPort>,
    guest: &Arc<Mutex<G>>,
    completion_errors: &Arc<Mutex<Vec<IdeControllerError>>>,
    channel: IdeChannelId,
    context: &mut ParallelSchedulerContext<'_>,
) where
    G: IdeControllerGuestMemory,
{
    let mut controller = controller.lock().expect("IDE controller timing lock");
    {
        let mut guest = guest.lock().expect("IDE DMA guest memory lock");
        if let Err(error) = controller.execute_dma(channel, &mut *guest) {
            completion_errors
                .lock()
                .expect("IDE timing completion error lock")
                .push(error);
            return;
        }
    }
    if let Some(interrupt_port) = interrupt_port {
        if let Err(error) = interrupt_port.sync_controller_parallel(context, &controller) {
            completion_errors
                .lock()
                .expect("IDE timing completion error lock")
                .push(error);
        }
    }
}

fn dma_delay_ticks(
    controller: &Arc<Mutex<IdeController>>,
    channel: IdeChannelId,
    base_delay: Tick,
) -> Result<Tick, IdeControllerError> {
    let controller = controller.lock().expect("IDE controller timing lock");
    let snapshot = controller.snapshot();
    let channel_snapshot = snapshot.channel(channel);
    if channel_snapshot.bmi().command() & IDE_BMI_COMMAND_START == 0 {
        return Err(IdeControllerError::DmaNotActive { channel });
    }
    let disk = match channel_snapshot.selected_device() {
        IdeDeviceId::Device0 => channel_snapshot.device0(),
        IdeDeviceId::Device1 => channel_snapshot.device1(),
    }
    .ok_or(IdeControllerError::NoSelectedDevice {
        channel,
        device: channel_snapshot.selected_device(),
    })?;
    let sectors = match disk.transfer() {
        Some(IdeDiskTransferSnapshot::Dma { sectors, .. }) => *sectors,
        _ => {
            return Err(IdeControllerError::Disk {
                channel,
                source: IdeDiskError::DmaNotReady {
                    command: disk.task_file().command(),
                },
            });
        }
    };
    base_delay
        .checked_add(sectors)
        .ok_or(rem6_kernel::SchedulerError::TickOverflow {
            now: base_delay,
            delay: sectors,
        })
        .map_err(scheduler_error)
}

fn scheduler_error(source: rem6_kernel::SchedulerError) -> IdeControllerError {
    IdeControllerError::Scheduler { source }
}
