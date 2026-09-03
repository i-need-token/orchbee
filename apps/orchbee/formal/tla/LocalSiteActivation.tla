----------------------- MODULE LocalSiteActivation -----------------------
EXTENDS FiniteSets, Naturals, TLC

\* Client-side refinement of the accepted local Site connect-first and lazy
\* activation contract. Exactly two Client slots are checked. External
\* launcher and cohort outcomes are deliberately unfair: the only progress
\* assumption is the Client-owned total-deadline observation. The model does
\* not select process daemonization, service management, rendezvous paths,
\* ACL policy, cleanup, recovery, a numeric deadline, or public wiring.

CONSTANTS Clients, NoClient

ASSUME /\ Cardinality(Clients) = 2
       /\ NoClient \notin Clients

ClientPhases == {
  "idle", "connecting", "lock-ready", "lock-held",
  "waiting-readiness", "connected", "failed", "information", "exited"
}

ClientResults == {
  "none", "connected", "help", "version", "incompatible",
  "partial-cohort", "timeout", "launcher-failure"
}

FailureResults == {
  "incompatible", "partial-cohort", "timeout", "launcher-failure"
}

ConnectionPaths == {"none", "present", "activation"}
PendingTuiPhases == {
  "connecting", "lock-ready", "lock-held", "waiting-readiness"
}
SettledTuiPhases == {"connected", "failed", "exited"}

InitialSitePhases == {"absent", "ready", "incompatible", "partial"}
SitePhases == InitialSitePhases \cup {"launching"}
ReadinessResults == {
  "none", "ready", "incompatible", "partial", "launcher-failure"
}
CohortValues == {"none", "exact-installed"}
ServiceLifetimeOwners == {"none", "site"}

Actions == {
  "init", "invoke-tui", "complete-help", "complete-version",
  "connect-finds-compatible", "connect-finds-absent",
  "connect-finds-incompatible", "connect-finds-partial",
  "acquire-activation-lock", "activation-lock-contended",
  "launch-exact-installed-cohort", "recheck-compatible",
  "recheck-activation-pending", "recheck-incompatible",
  "recheck-partial", "recheck-launcher-failure",
  "cohort-becomes-ready", "cohort-becomes-incompatible",
  "cohort-becomes-partial", "launcher-fails",
  "observe-ready", "observe-incompatible", "observe-partial",
  "observe-launcher-failure", "deadline-expires", "exit-client"
}

VARIABLES
  clientPhase,
  clientResult,
  connectionPath,
  sitePhase,
  readinessResult,
  activationOwner,
  launchCount,
  launchedCohort,
  serviceLifetimeOwner,
  priorSitePhase,
  priorReadinessResult,
  priorActivationOwner,
  priorLaunchCount,
  priorLaunchedCohort,
  priorServiceLifetimeOwner,
  lastAction,
  lastClient

DomainVars == <<
  clientPhase,
  clientResult,
  connectionPath,
  sitePhase,
  readinessResult,
  activationOwner,
  launchCount,
  launchedCohort,
  serviceLifetimeOwner
>>

AuditVars == <<
  priorSitePhase,
  priorReadinessResult,
  priorActivationOwner,
  priorLaunchCount,
  priorLaunchedCohort,
  priorServiceLifetimeOwner,
  lastAction,
  lastClient
>>

vars == <<DomainVars, AuditVars>>

SetClient(client, phase, result, path) ==
  /\ clientPhase' = [clientPhase EXCEPT ![client] = phase]
  /\ clientResult' = [clientResult EXCEPT ![client] = result]
  /\ connectionPath' = [connectionPath EXCEPT ![client] = path]

Record(action, client) ==
  /\ priorSitePhase' = sitePhase
  /\ priorReadinessResult' = readinessResult
  /\ priorActivationOwner' = activationOwner
  /\ priorLaunchCount' = launchCount
  /\ priorLaunchedCohort' = launchedCohort
  /\ priorServiceLifetimeOwner' = serviceLifetimeOwner
  /\ lastAction' = action
  /\ lastClient' = client

Init ==
  /\ clientPhase = [client \in Clients |-> "idle"]
  /\ clientResult = [client \in Clients |-> "none"]
  /\ connectionPath = [client \in Clients |-> "none"]
  /\ sitePhase \in InitialSitePhases
  /\ readinessResult = "none"
  /\ activationOwner = NoClient
  /\ launchCount = 0
  /\ launchedCohort = "none"
  /\ serviceLifetimeOwner =
       IF sitePhase = "ready" THEN "site" ELSE "none"
  /\ priorSitePhase = sitePhase
  /\ priorReadinessResult = readinessResult
  /\ priorActivationOwner = activationOwner
  /\ priorLaunchCount = launchCount
  /\ priorLaunchedCohort = launchedCohort
  /\ priorServiceLifetimeOwner = serviceLifetimeOwner
  /\ lastAction = "init"
  /\ lastClient = NoClient

InvokeTui(client) ==
  /\ client \in Clients
  /\ clientPhase[client] = "idle"
  /\ SetClient(client, "connecting", "none", "none")
  /\ UNCHANGED <<sitePhase, readinessResult, activationOwner, launchCount,
       launchedCohort, serviceLifetimeOwner>>
  /\ Record("invoke-tui", client)

CompleteHelp(client) ==
  /\ client \in Clients
  /\ clientPhase[client] = "idle"
  /\ SetClient(client, "information", "help", "none")
  /\ UNCHANGED <<sitePhase, readinessResult, activationOwner, launchCount,
       launchedCohort, serviceLifetimeOwner>>
  /\ Record("complete-help", client)

CompleteVersion(client) ==
  /\ client \in Clients
  /\ clientPhase[client] = "idle"
  /\ SetClient(client, "information", "version", "none")
  /\ UNCHANGED <<sitePhase, readinessResult, activationOwner, launchCount,
       launchedCohort, serviceLifetimeOwner>>
  /\ Record("complete-version", client)

ConnectFindsCompatible(client) ==
  /\ clientPhase[client] = "connecting"
  /\ sitePhase = "ready"
  /\ SetClient(client, "connected", "connected", "present")
  /\ UNCHANGED <<sitePhase, readinessResult, activationOwner, launchCount,
       launchedCohort, serviceLifetimeOwner>>
  /\ Record("connect-finds-compatible", client)

ConnectFindsAbsent(client) ==
  /\ clientPhase[client] = "connecting"
  /\ sitePhase \in {"absent", "launching"}
  /\ SetClient(client, "lock-ready", "none", "none")
  /\ UNCHANGED <<sitePhase, readinessResult, activationOwner, launchCount,
       launchedCohort, serviceLifetimeOwner>>
  /\ Record("connect-finds-absent", client)

ConnectFindsIncompatible(client) ==
  /\ clientPhase[client] = "connecting"
  /\ sitePhase = "incompatible"
  /\ SetClient(client, "failed", "incompatible", "none")
  /\ UNCHANGED <<sitePhase, readinessResult, activationOwner, launchCount,
       launchedCohort, serviceLifetimeOwner>>
  /\ Record("connect-finds-incompatible", client)

ConnectFindsPartial(client) ==
  /\ clientPhase[client] = "connecting"
  /\ sitePhase = "partial"
  /\ SetClient(client, "failed", "partial-cohort", "none")
  /\ UNCHANGED <<sitePhase, readinessResult, activationOwner, launchCount,
       launchedCohort, serviceLifetimeOwner>>
  /\ Record("connect-finds-partial", client)

AcquireActivationLock(client) ==
  /\ clientPhase[client] = "lock-ready"
  /\ activationOwner = NoClient
  /\ SetClient(client, "lock-held", "none", "none")
  /\ activationOwner' = client
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("acquire-activation-lock", client)

ActivationLockContended(client) ==
  /\ clientPhase[client] = "lock-ready"
  /\ activationOwner \in Clients \ {client}
  /\ SetClient(client, "waiting-readiness", "none", "none")
  /\ UNCHANGED <<sitePhase, readinessResult, activationOwner, launchCount,
       launchedCohort, serviceLifetimeOwner>>
  /\ Record("activation-lock-contended", client)

LaunchExactInstalledCohort(client) ==
  /\ clientPhase[client] = "lock-held"
  /\ activationOwner = client
  /\ sitePhase = "absent"
  /\ readinessResult = "none"
  /\ launchCount = 0
  /\ SetClient(client, "waiting-readiness", "none", "none")
  /\ sitePhase' = "launching"
  /\ launchCount' = 1
  /\ launchedCohort' = "exact-installed"
  /\ UNCHANGED <<readinessResult, activationOwner, serviceLifetimeOwner>>
  /\ Record("launch-exact-installed-cohort", client)

RecheckCompatible(client) ==
  /\ clientPhase[client] = "lock-held"
  /\ activationOwner = client
  /\ sitePhase = "ready"
  /\ SetClient(client, "connected", "connected", "activation")
  /\ activationOwner' = NoClient
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("recheck-compatible", client)

RecheckActivationPending(client) ==
  /\ clientPhase[client] = "lock-held"
  /\ activationOwner = client
  /\ sitePhase = "launching"
  /\ readinessResult = "none"
  /\ SetClient(client, "waiting-readiness", "none", "none")
  /\ activationOwner' = NoClient
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("recheck-activation-pending", client)

RecheckIncompatible(client) ==
  /\ clientPhase[client] = "lock-held"
  /\ activationOwner = client
  /\ \/ sitePhase = "incompatible"
     \/ readinessResult = "incompatible"
  /\ SetClient(client, "failed", "incompatible", "none")
  /\ activationOwner' = NoClient
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("recheck-incompatible", client)

RecheckPartial(client) ==
  /\ clientPhase[client] = "lock-held"
  /\ activationOwner = client
  /\ \/ sitePhase = "partial"
     \/ readinessResult = "partial"
  /\ SetClient(client, "failed", "partial-cohort", "none")
  /\ activationOwner' = NoClient
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("recheck-partial", client)

RecheckLauncherFailure(client) ==
  /\ clientPhase[client] = "lock-held"
  /\ activationOwner = client
  /\ readinessResult = "launcher-failure"
  /\ SetClient(client, "failed", "launcher-failure", "none")
  /\ activationOwner' = NoClient
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("recheck-launcher-failure", client)

CohortBecomesReady ==
  /\ sitePhase = "launching"
  /\ readinessResult = "none"
  /\ sitePhase' = "ready"
  /\ readinessResult' = "ready"
  /\ serviceLifetimeOwner' = "site"
  /\ UNCHANGED <<clientPhase, clientResult, connectionPath,
       activationOwner, launchCount, launchedCohort>>
  /\ Record("cohort-becomes-ready", NoClient)

CohortBecomesIncompatible ==
  /\ sitePhase = "launching"
  /\ readinessResult = "none"
  /\ sitePhase' = "incompatible"
  /\ readinessResult' = "incompatible"
  /\ UNCHANGED <<clientPhase, clientResult, connectionPath,
       activationOwner, launchCount, launchedCohort, serviceLifetimeOwner>>
  /\ Record("cohort-becomes-incompatible", NoClient)

CohortBecomesPartial ==
  /\ sitePhase = "launching"
  /\ readinessResult = "none"
  /\ sitePhase' = "partial"
  /\ readinessResult' = "partial"
  /\ UNCHANGED <<clientPhase, clientResult, connectionPath,
       activationOwner, launchCount, launchedCohort, serviceLifetimeOwner>>
  /\ Record("cohort-becomes-partial", NoClient)

LauncherFails ==
  /\ sitePhase = "launching"
  /\ readinessResult = "none"
  /\ readinessResult' = "launcher-failure"
  /\ UNCHANGED <<clientPhase, clientResult, connectionPath, sitePhase,
       activationOwner, launchCount, launchedCohort, serviceLifetimeOwner>>
  /\ Record("launcher-fails", NoClient)

ObserveReady(client) ==
  /\ clientPhase[client] = "waiting-readiness"
  /\ readinessResult = "ready"
  /\ sitePhase = "ready"
  /\ SetClient(client, "connected", "connected", "activation")
  /\ activationOwner' =
       IF activationOwner = client THEN NoClient ELSE activationOwner
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("observe-ready", client)

ObserveIncompatible(client) ==
  /\ clientPhase[client] = "waiting-readiness"
  /\ readinessResult = "incompatible"
  /\ SetClient(client, "failed", "incompatible", "none")
  /\ activationOwner' =
       IF activationOwner = client THEN NoClient ELSE activationOwner
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("observe-incompatible", client)

ObservePartial(client) ==
  /\ clientPhase[client] = "waiting-readiness"
  /\ readinessResult = "partial"
  /\ SetClient(client, "failed", "partial-cohort", "none")
  /\ activationOwner' =
       IF activationOwner = client THEN NoClient ELSE activationOwner
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("observe-partial", client)

ObserveLauncherFailure(client) ==
  /\ clientPhase[client] = "waiting-readiness"
  /\ readinessResult = "launcher-failure"
  /\ SetClient(client, "failed", "launcher-failure", "none")
  /\ activationOwner' =
       IF activationOwner = client THEN NoClient ELSE activationOwner
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("observe-launcher-failure", client)

DeadlineExpires(client) ==
  /\ clientPhase[client] \in PendingTuiPhases
  /\ SetClient(client, "failed", "timeout", "none")
  /\ activationOwner' =
       IF activationOwner = client THEN NoClient ELSE activationOwner
  /\ UNCHANGED <<sitePhase, readinessResult, launchCount, launchedCohort,
       serviceLifetimeOwner>>
  /\ Record("deadline-expires", client)

ExitClient(client) ==
  /\ clientPhase[client] = "connected"
  /\ SetClient(client, "exited", "connected", connectionPath[client])
  /\ UNCHANGED <<sitePhase, readinessResult, activationOwner, launchCount,
       launchedCohort, serviceLifetimeOwner>>
  /\ Record("exit-client", client)

Next ==
  \/ \E client \in Clients: InvokeTui(client)
  \/ \E client \in Clients: CompleteHelp(client)
  \/ \E client \in Clients: CompleteVersion(client)
  \/ \E client \in Clients: ConnectFindsCompatible(client)
  \/ \E client \in Clients: ConnectFindsAbsent(client)
  \/ \E client \in Clients: ConnectFindsIncompatible(client)
  \/ \E client \in Clients: ConnectFindsPartial(client)
  \/ \E client \in Clients: AcquireActivationLock(client)
  \/ \E client \in Clients: ActivationLockContended(client)
  \/ \E client \in Clients: LaunchExactInstalledCohort(client)
  \/ \E client \in Clients: RecheckCompatible(client)
  \/ \E client \in Clients: RecheckActivationPending(client)
  \/ \E client \in Clients: RecheckIncompatible(client)
  \/ \E client \in Clients: RecheckPartial(client)
  \/ \E client \in Clients: RecheckLauncherFailure(client)
  \/ CohortBecomesReady
  \/ CohortBecomesIncompatible
  \/ CohortBecomesPartial
  \/ LauncherFails
  \/ \E client \in Clients: ObserveReady(client)
  \/ \E client \in Clients: ObserveIncompatible(client)
  \/ \E client \in Clients: ObservePartial(client)
  \/ \E client \in Clients: ObserveLauncherFailure(client)
  \/ \E client \in Clients: DeadlineExpires(client)
  \/ \E client \in Clients: ExitClient(client)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ \A client \in Clients: WF_vars(DeadlineExpires(client))

TypeOK ==
  /\ clientPhase \in [Clients -> ClientPhases]
  /\ clientResult \in [Clients -> ClientResults]
  /\ connectionPath \in [Clients -> ConnectionPaths]
  /\ sitePhase \in SitePhases
  /\ readinessResult \in ReadinessResults
  /\ activationOwner \in Clients \cup {NoClient}
  /\ launchCount \in 0..1
  /\ launchedCohort \in CohortValues
  /\ serviceLifetimeOwner \in ServiceLifetimeOwners
  /\ priorSitePhase \in SitePhases
  /\ priorReadinessResult \in ReadinessResults
  /\ priorActivationOwner \in Clients \cup {NoClient}
  /\ priorLaunchCount \in 0..1
  /\ priorLaunchedCohort \in CohortValues
  /\ priorServiceLifetimeOwner \in ServiceLifetimeOwners
  /\ lastAction \in Actions
  /\ lastClient \in Clients \cup {NoClient}

ClientShapeIsExact ==
  \A client \in Clients:
    /\ (clientPhase[client] \in {
          "idle", "connecting", "lock-ready", "lock-held",
          "waiting-readiness"
        } =>
          /\ clientResult[client] = "none"
          /\ connectionPath[client] = "none")
    /\ (clientPhase[client] = "information" =>
          /\ clientResult[client] \in {"help", "version"}
          /\ connectionPath[client] = "none")
    /\ (clientPhase[client] \in {"connected", "exited"} =>
          /\ clientResult[client] = "connected"
          /\ connectionPath[client] \in {"present", "activation"})
    /\ (clientPhase[client] = "failed" =>
          /\ clientResult[client] \in FailureResults
          /\ connectionPath[client] = "none")

ActivationOwnerIsUniqueAndPending ==
  /\ activationOwner = NoClient \/
       clientPhase[activationOwner] \in {"lock-held", "waiting-readiness"}
  /\ \A client \in Clients:
       clientPhase[client] = "lock-held" => activationOwner = client

LaunchIsSingletonAndExact ==
  (launchCount = 0 /\ launchedCohort = "none") \/
  (launchCount = 1 /\ launchedCohort = "exact-installed")

LaunchingStateHasOneExactAttempt ==
  sitePhase = "launching" =>
    /\ launchCount = 1
    /\ launchedCohort = "exact-installed"

ReadinessResultMatchesLaunch ==
  /\ (readinessResult # "none" => launchCount = 1)
  /\ (readinessResult = "ready" => sitePhase = "ready")
  /\ (readinessResult = "incompatible" => sitePhase = "incompatible")
  /\ (readinessResult = "partial" => sitePhase = "partial")
  /\ (readinessResult = "launcher-failure" => sitePhase = "launching")

LaunchStartsOnlyUnderTheActivationLock ==
  lastAction = "launch-exact-installed-cohort" =>
    /\ lastClient \in Clients
    /\ priorActivationOwner = lastClient
    /\ activationOwner = lastClient
    /\ priorLaunchCount = 0
    /\ launchCount = 1
    /\ launchedCohort = "exact-installed"

ContenderWaitsForTypedReadiness ==
  lastAction = "activation-lock-contended" =>
    /\ lastClient \in Clients
    /\ clientPhase[lastClient] = "waiting-readiness"
    /\ activationOwner # lastClient

ConnectedClientsOwnNoActivationResource ==
  \A client \in Clients:
    clientPhase[client] \in {"connected", "exited"} =>
      activationOwner # client

CompatibleSiteIsRequiredForConnection ==
  (\E client \in Clients:
     clientPhase[client] \in {"connected", "exited"}) =>
    sitePhase = "ready"

ReadySiteOwnsItsServiceLifetime ==
  sitePhase = "ready" => serviceLifetimeOwner = "site"

NoClientSupervisesServiceLifetime ==
  serviceLifetimeOwner \notin Clients

ConnectionProbeNeverActivates ==
  lastAction \in {
    "connect-finds-compatible", "connect-finds-absent",
    "connect-finds-incompatible", "connect-finds-partial"
  } =>
    /\ sitePhase = priorSitePhase
    /\ readinessResult = priorReadinessResult
    /\ activationOwner = priorActivationOwner
    /\ launchCount = priorLaunchCount
    /\ launchedCohort = priorLaunchedCohort
    /\ serviceLifetimeOwner = priorServiceLifetimeOwner

InformationInvocationHasNoActivationEffect ==
  lastAction \in {"complete-help", "complete-version"} =>
    /\ sitePhase = priorSitePhase
    /\ readinessResult = priorReadinessResult
    /\ activationOwner = priorActivationOwner
    /\ launchCount = priorLaunchCount
    /\ launchedCohort = priorLaunchedCohort
    /\ serviceLifetimeOwner = priorServiceLifetimeOwner

ExplicitFailureHasNoServiceStopEffect ==
  lastAction \in {
    "connect-finds-incompatible", "connect-finds-partial",
    "recheck-incompatible", "recheck-partial",
    "recheck-launcher-failure", "observe-incompatible",
    "observe-partial", "observe-launcher-failure", "deadline-expires"
  } =>
    /\ sitePhase = priorSitePhase
    /\ launchCount = priorLaunchCount
    /\ launchedCohort = priorLaunchedCohort
    /\ serviceLifetimeOwner = priorServiceLifetimeOwner

SettledClientReleasesItsActivationResource ==
  lastClient \in Clients /\
  clientPhase[lastClient] \in {"connected", "failed", "exited"} =>
    activationOwner # lastClient

ClientExitPreservesSite ==
  lastAction = "exit-client" =>
    /\ sitePhase = priorSitePhase
    /\ readinessResult = priorReadinessResult
    /\ activationOwner = priorActivationOwner
    /\ launchCount = priorLaunchCount
    /\ launchedCohort = priorLaunchedCohort
    /\ serviceLifetimeOwner = priorServiceLifetimeOwner

EveryPendingTuiEventuallySettles ==
  \A client \in Clients:
    [](clientPhase[client] \in PendingTuiPhases =>
       <>(clientPhase[client] \in SettledTuiPhases))

ActivationResourceEventuallyReleases ==
  [](activationOwner # NoClient => <> (activationOwner = NoClient))

=============================================================================
