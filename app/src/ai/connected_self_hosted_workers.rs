use warpui::{Entity, ModelContext, SingletonEntity};

use crate::server::server_api::ai::ConnectedSelfHostedWorker;

pub const WARP_WORKER_HOST: &str = "warp";

pub enum ConnectedSelfHostedWorkersEvent {
    Changed,
}

pub struct ConnectedSelfHostedWorkersModel {
    workers: Vec<ConnectedSelfHostedWorker>,
}

impl ConnectedSelfHostedWorkersModel {
    pub fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self {
            workers: Vec::new(),
        }
    }

    pub fn worker_hosts_excluding(&self, excluded: Option<&str>) -> Vec<String> {
        let mut hosts: Vec<String> = self
            .workers
            .iter()
            .map(|worker| worker.worker_host.clone())
            .filter(|host| !host.trim().is_empty())
            .filter(|host| !host.eq_ignore_ascii_case(WARP_WORKER_HOST))
            .filter(|host| match excluded {
                Some(excluded) => !host.eq_ignore_ascii_case(excluded),
                None => true,
            })
            .collect();
        hosts.sort();
        hosts.dedup();
        hosts
    }

    pub fn refresh(&mut self, ctx: &mut ModelContext<Self>) {
        self.clear_workers(ctx);
    }

    fn clear_workers(&mut self, ctx: &mut ModelContext<Self>) {
        if self.clear_worker_cache() {
            ctx.emit(ConnectedSelfHostedWorkersEvent::Changed);
        }
    }

    fn clear_worker_cache(&mut self) -> bool {
        if self.workers.is_empty() {
            return false;
        }
        self.workers.clear();
        true
    }
}

impl Entity for ConnectedSelfHostedWorkersModel {
    type Event = ConnectedSelfHostedWorkersEvent;
}

impl SingletonEntity for ConnectedSelfHostedWorkersModel {}

#[cfg(test)]
#[path = "connected_self_hosted_workers_tests.rs"]
mod tests;
