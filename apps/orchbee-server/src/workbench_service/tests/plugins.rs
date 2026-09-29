use super::*;
use orchbee_workbench_protocol::{
    PluginAction, PluginFailure, PluginInvocationContext, PluginOutcome, PluginRequest, SiteId,
};
#[test]
fn plugin_execution_requires_read_create_and_lifecycle_authority_before_resolution() {
    for read in [false, true] {
        for create in [false, true] {
            for lifecycle in [false, true] {
                let terminals = Arc::new(LocalTerminalService::new(Arc::new(Mutex::new(
                    WorkerRegistry::new(),
                ))));
                let service = WorkbenchService::new(
                    WorkbenchPolicy {
                        read,
                        create_terminal: create,
                        manage_terminal_lifecycle: lifecycle,
                        manage_navigation: true,
                        associate_directory: true,
                    },
                    terminals,
                );
                let request = PluginRequest {
                    protocol_version: ProtocolVersion,
                    request_id: WorkbenchRequestId::parse(&"1".repeat(32)).unwrap(),
                    site_id: SiteId::parse(&"2".repeat(32)).unwrap(),
                    action: PluginAction::Invoke {
                        operation_id: "4".repeat(32),
                        plugin_id: None,
                        action_id: "hello".into(),
                        context: Box::new(PluginInvocationContext::default()),
                    },
                };
                let WorkbenchResponse::Plugin(result) =
                    response(service.execute(WorkbenchRequest::Plugin(request)))
                else {
                    panic!()
                };
                assert_eq!(
                    result.outcome,
                    PluginOutcome::Failed {
                        reason: if read && create && lifecycle {
                            PluginFailure::SiteMismatch
                        } else {
                            PluginFailure::Denied
                        }
                    }
                );
            }
        }
    }
}
