use warpui::{Entity, ModelContext, SingletonEntity};

#[derive(Clone)]
pub struct TeamTesterStatus {}

impl TeamTesterStatus {
    #[cfg(test)]
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self {}
    }

    #[cfg(test)]
    pub fn mock(ctx: &mut ModelContext<Self>) -> Self {
        Self::new(ctx)
    }

    /// Emit an event to start or force-refresh the remaining cloud-object poller.
    pub fn initiate_data_pollers(&mut self, force_refresh: bool, ctx: &mut ModelContext<Self>) {
        let event = if force_refresh {
            TeamTesterStatusEvent::ForceRefreshDataPollers
        } else {
            TeamTesterStatusEvent::InitiateDataPollers
        };
        ctx.emit(event)
    }
}

pub enum TeamTesterStatusEvent {
    InitiateDataPollers,
    ForceRefreshDataPollers,
}

impl Entity for TeamTesterStatus {
    type Event = TeamTesterStatusEvent;
}

impl SingletonEntity for TeamTesterStatus {}
