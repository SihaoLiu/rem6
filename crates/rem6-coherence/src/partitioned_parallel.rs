use std::collections::BTreeMap;
use std::sync::Arc;

use rem6_cache::CacheControllerError;
use rem6_transport::{TargetOutcome, TransportError};

use super::deferred::{DeferredMemoryPath, DeferredMemoryWork, DeferredWaitFor};
use super::snoop::{DirectorySnoopWork, SnoopRoute};
use super::{
    decision_uses_backing_memory, map_cache_error, partitioned_directory_response, response_record,
    DirectoryDecisionRecord, HarnessError, PartitionedDirectoryLineHarness, SubmitKind,
    SubmitResult,
};
use rem6_memory::{AgentId, MemoryRequest};

impl PartitionedDirectoryLineHarness {
    pub fn submit_cpu_request_parallel(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<SubmitResult, HarnessError> {
        let cache = self.cache_arc(agent)?;
        let request_id = request.id();
        let result = match cache
            .lock()
            .expect("cache lock")
            .accept_cpu_request(request)
        {
            Ok(result) => result,
            Err(CacheControllerError::LineBusy { state }) => {
                self.wait_for.record_cache_busy(
                    agent,
                    self.line.address().get(),
                    request_id,
                    self.scheduler.now(),
                );
                return Err(HarnessError::LineBusy { state });
            }
            Err(error) => return Err(map_cache_error(error)),
        };
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.cpu_responses
                .lock()
                .expect("response lock")
                .push(response_record(
                    self.scheduler.now(),
                    cache_result,
                    response,
                ));
            return Ok(SubmitResult::new(SubmitKind::ImmediateHit, cache_result));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(HarnessError::Cache(CacheControllerError::NoPendingMiss))?;
        let route = *self
            .routes
            .get(&agent)
            .ok_or(HarnessError::UnknownCache { agent })?;
        let cache_route = self
            .transport
            .route(route)
            .cloned()
            .ok_or(HarnessError::Transport(TransportError::UnknownRoute {
                route,
            }))?;
        let mut cache_routes = BTreeMap::new();
        for (agent, route_id) in &self.routes {
            let route_info =
                self.transport
                    .route(*route_id)
                    .cloned()
                    .ok_or(HarnessError::Transport(TransportError::UnknownRoute {
                        route: *route_id,
                    }))?;
            cache_routes.insert(*agent, SnoopRoute::new(*route_id, route_info));
        }

        let directory = Arc::clone(&self.directory);
        let caches = self.caches.clone();
        let backing = self.backing.clone();
        let dram_memory = self.dram_memory.clone();
        let decisions = Arc::clone(&self.directory_decisions);
        let dram_accesses = Arc::clone(&self.dram_accesses);
        let fabric = self.fabric.clone();
        let trace = self.trace.clone();
        let response_cache = Arc::clone(&cache);
        let responses = Arc::clone(&self.cpu_responses);
        let wait_for = self.wait_for.clone();
        let line = self.line;
        let memory_path = self.memory_route.zip(self.memory_route_info.clone()).map(
            |(memory_route, memory_route_info)| DeferredMemoryPath {
                cache_route_id: route,
                cache_route: cache_route.clone(),
                memory_route_id: memory_route,
                memory_route: memory_route_info,
            },
        );
        let deferred = memory_path.map(|path| DeferredMemoryWork {
            path,
            cache_routes: cache_routes.clone(),
            caches: caches.clone(),
            backing: backing.clone(),
            dram_memory: dram_memory.clone(),
            fabric: fabric.clone(),
            trace: trace.clone(),
            response_cache: Arc::clone(&response_cache),
            responses: Arc::clone(&responses),
            decisions: Arc::clone(&decisions),
            dram_accesses: Arc::clone(&dram_accesses),
            wait_for: Some(DeferredWaitFor::new(wait_for.clone(), line)),
        });
        let response_cache_for_snoop = Arc::clone(&response_cache);
        let responses_for_snoop = Arc::clone(&responses);

        self.transport
            .submit_parallel(
                &mut self.scheduler,
                route,
                downstream,
                trace.clone(),
                move |delivery, context| {
                    let decision = directory
                        .lock()
                        .expect("directory lock")
                        .accept(delivery.request().clone())
                        .expect("directory decision");
                    if decision_uses_backing_memory(&decision) {
                        if let Some(deferred) = deferred {
                            deferred
                                .schedule_parallel(context, delivery.request().clone(), decision)
                                .expect("deferred memory response");
                            return TargetOutcome::NoResponse;
                        }
                    }
                    if !decision.snoops().is_empty() && !decision_uses_backing_memory(&decision) {
                        DirectorySnoopWork::new(
                            delivery.request().clone(),
                            decision,
                            SnoopRoute::new(route, cache_route.clone()),
                            cache_routes,
                            caches,
                            fabric,
                            trace.clone(),
                            Arc::clone(&response_cache_for_snoop),
                            Arc::clone(&responses_for_snoop),
                            Arc::clone(&decisions),
                        )
                        .schedule_parallel(context, delivery.tick())
                        .expect("scheduled directory snoops");
                        return TargetOutcome::NoResponse;
                    }
                    let response = partitioned_directory_response(
                        delivery.request(),
                        &decision,
                        &caches,
                        &backing,
                    )
                    .expect("directory response");
                    decisions
                        .lock()
                        .expect("decision lock")
                        .push(DirectoryDecisionRecord::new(
                            delivery.tick(),
                            delivery.request().id().agent(),
                            decision,
                        ));
                    TargetOutcome::Respond(response)
                },
                move |delivery| {
                    let response_request = delivery.response().request_id();
                    let result = response_cache
                        .lock()
                        .expect("cache lock")
                        .accept_fill(delivery.response().clone())
                        .expect("cache fill");
                    wait_for.clear_cache_line(response_request.agent(), line.address().get());
                    if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                        responses
                            .lock()
                            .expect("response lock")
                            .push(response_record(delivery.tick(), result.kind(), response));
                    }
                },
            )
            .map_err(HarnessError::Transport)?;

        Ok(SubmitResult::new(SubmitKind::ScheduledMiss, cache_result))
    }
}
