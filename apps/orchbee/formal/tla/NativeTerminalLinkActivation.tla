---------------- MODULE NativeTerminalLinkActivation ----------------
EXTENDS FiniteSets, Naturals, TLC

CONSTANTS MaxClick, MaxVersion
ASSUME MaxClick = 2 /\ MaxVersion = 2

PointerOwners == {"idle", "pane", "selection", "pending", "link", "link-cancelled"}
Phases == {"idle", "resolving", "selected", "browser-ready",
           "plugin-submitted", "browser-submitted", "settled"}
Effects == {"none", "plugin", "browser"}
Urls == {"none", "http", "other"}
Handlers == {"unknown", "matched", "none"}

VARIABLE s
vars == <<s>>

Init ==
  s = [lifecycle |-> "active", pointer |-> "idle", mapValid |-> TRUE,
       contentVersion |-> 1, paintVersion |-> 1, pressedVersion |-> 0,
       job |-> 0, sourceVersion |-> 0, phase |-> "idle", url |-> "none",
       handler |-> "unknown", registryStable |-> TRUE, effect |-> "none",
       pluginEligible |-> {}, browserEligible |-> {}, pluginAttempts |-> {},
       registeredSelections |-> {}, serverGuarded |-> {}, browserAttempts |-> {},
       staleRejected |-> {}, registryRejected |-> {},
       settled |-> {}, busyRefusals |-> 0, linkForwards |-> 0,
       controlRequests |-> 0, coverage |-> "Init"]

CtrlPress ==
  /\ s.lifecycle = "active" /\ s.pointer = "idle" /\ s.mapValid
  /\ s.phase \in {"idle", "settled"} /\ s.job < MaxClick
  /\ s' = [s EXCEPT !.pointer = "link", !.pressedVersion = s.paintVersion,
                    !.coverage = "CtrlPress"]

BusyCtrlPress ==
  /\ s.lifecycle = "active" /\ s.pointer = "idle"
  /\ s.phase \notin {"idle", "settled"} /\ s.busyRefusals < MaxClick
  /\ s' = [s EXCEPT !.busyRefusals = @ + 1, !.coverage = "BusyCtrlPress"]

OrdinaryPress ==
  /\ s.lifecycle = "active" /\ s.pointer = "idle"
  /\ \E owner \in {"pane", "selection", "pending"}:
       s' = [s EXCEPT !.pointer = owner, !.coverage = "OrdinaryPress"]

ModifierChange ==
  /\ s.lifecycle = "active" /\ s.pointer # "idle"
  /\ s' = [s EXCEPT !.coverage = "ModifierChange"]

OrdinaryRelease ==
  /\ s.pointer \in {"pane", "selection", "pending"}
  /\ s' = [s EXCEPT !.pointer = "idle", !.coverage = "OrdinaryRelease"]

CancelLinkMovement ==
  /\ s.pointer = "link"
  /\ s' = [s EXCEPT !.pointer = "link-cancelled",
                    !.coverage = "CancelLinkMovement"]

MismatchedLinkRelease ==
  /\ s.pointer \in {"link", "link-cancelled"}
  /\ s' = [s EXCEPT !.pointer = "idle", !.pressedVersion = 0,
                    !.coverage = "MismatchedLinkRelease"]

ReleaseLink ==
  /\ s.lifecycle = "active" /\ s.pointer = "link" /\ s.mapValid
  /\ s.pressedVersion = s.paintVersion /\ s.phase \in {"idle", "settled"}
  /\ s.job < MaxClick
  /\ s' = [s EXCEPT !.pointer = "idle", !.pressedVersion = 0,
                    !.job = @ + 1, !.sourceVersion = s.pressedVersion,
                    !.phase = "resolving", !.url = "none",
                    !.handler = "unknown", !.registryStable = TRUE,
                    !.effect = "none", !.coverage = "ReleaseLink"]

ContentChanges ==
  /\ s.lifecycle = "active" /\ s.contentVersion < MaxVersion
  /\ s' = [s EXCEPT !.contentVersion = @ + 1, !.mapValid = FALSE,
                    !.pointer = IF s.pointer = "link" THEN "link-cancelled" ELSE @,
                    !.coverage = "ContentChanges"]

InvalidateMap ==
  /\ s.lifecycle = "active" /\ s.mapValid
  /\ s' = [s EXCEPT !.mapValid = FALSE,
                    !.pointer = IF s.pointer = "link" THEN "link-cancelled" ELSE @,
                    !.coverage = "InvalidateMap"]

SuccessfulDraw ==
  /\ s.lifecycle = "active" /\ ~s.mapValid
  /\ s' = [s EXCEPT !.paintVersion = s.contentVersion, !.mapValid = TRUE,
                    !.coverage = "SuccessfulDraw"]

RejectStaleSource ==
  /\ s.phase \in {"resolving", "selected", "browser-ready"}
  /\ s.sourceVersion # s.contentVersion
  /\ s' = [s EXCEPT !.phase = "settled", !.staleRejected = @ \cup {s.job},
                    !.settled = @ \cup {s.job}, !.coverage = "RejectStaleSource"]

ResolveNoLink ==
  /\ s.phase = "resolving" /\ s.sourceVersion = s.contentVersion
  /\ s' = [s EXCEPT !.phase = "settled", !.settled = @ \cup {s.job},
                    !.coverage = "ResolveNoLink"]

ResolveMatched ==
  /\ s.phase = "resolving" /\ s.sourceVersion = s.contentVersion
  /\ \E url \in {"http", "other"}:
       s' = [s EXCEPT !.phase = "selected", !.url = url,
                      !.handler = "matched",
                      !.pluginEligible = @ \cup {s.job},
                      !.registeredSelections = @ \cup {s.job},
                      !.coverage = "ResolveMatched"]

ResolveNoHandlerHttp ==
  /\ s.phase = "resolving" /\ s.sourceVersion = s.contentVersion
  /\ s' = [s EXCEPT !.phase = "browser-ready", !.url = "http",
                    !.handler = "none", !.browserEligible = @ \cup {s.job},
                    !.registeredSelections = @ \cup {s.job},
                    !.coverage = "ResolveNoHandlerHttp"]

ResolveNoHandlerOther ==
  /\ s.phase = "resolving" /\ s.sourceVersion = s.contentVersion
  /\ s' = [s EXCEPT !.phase = "settled", !.url = "other",
                    !.handler = "none", !.settled = @ \cup {s.job},
                    !.coverage = "ResolveNoHandlerOther"]

RegistryDrifts ==
  /\ s.phase \in {"selected", "browser-ready"} /\ s.registryStable
  /\ s' = [s EXCEPT !.registryStable = FALSE, !.coverage = "RegistryDrifts"]

RefuseRegistryDrift ==
  /\ s.phase \in {"selected", "browser-ready"} /\ ~s.registryStable
  /\ s' = [s EXCEPT !.phase = "settled",
                    !.registryRejected = @ \cup {s.job},
                    !.settled = @ \cup {s.job}, !.coverage = "RefuseRegistryDrift"]

SubmitPlugin ==
  /\ s.phase = "selected" /\ s.sourceVersion = s.contentVersion
  /\ s.registryStable /\ s.effect = "none" /\ s.job \in s.registeredSelections
  /\ s' = [s EXCEPT !.phase = "plugin-submitted", !.effect = "plugin",
                    !.pluginAttempts = @ \cup {s.job},
                    !.serverGuarded = @ \cup {s.job},
                    !.coverage = "SubmitPlugin"]

SettlePlugin ==
  /\ s.phase = "plugin-submitted" /\ s.lifecycle \in {"active", "draining"}
  /\ s' = [s EXCEPT !.phase = "settled", !.settled = @ \cup {s.job},
                    !.coverage = "SettlePlugin"]

LaunchBrowser ==
  /\ s.phase = "browser-ready" /\ s.sourceVersion = s.contentVersion
  /\ s.registryStable /\ s.effect = "none"
  /\ s' = [s EXCEPT !.phase = "browser-submitted", !.effect = "browser",
                    !.browserAttempts = @ \cup {s.job},
                    !.coverage = "LaunchBrowser"]

SettleBrowser ==
  /\ s.phase = "browser-submitted" /\ s.lifecycle \in {"active", "draining"}
  /\ s' = [s EXCEPT !.phase = "settled", !.settled = @ \cup {s.job},
                    !.coverage = "SettleBrowser"]

RequestShutdown ==
  /\ s.lifecycle = "active"
  /\ s' = [s EXCEPT !.lifecycle = "draining", !.pointer = "idle",
                    !.pressedVersion = 0,
                    !.phase = IF s.phase \in {"resolving", "selected", "browser-ready"}
                             THEN "settled" ELSE @,
                    !.settled = IF s.phase \in {"resolving", "selected", "browser-ready"}
                               THEN @ \cup {s.job} ELSE @,
                    !.coverage = "RequestShutdown"]

JoinLane ==
  /\ s.lifecycle = "draining" /\ s.phase \notin {"plugin-submitted", "browser-submitted"}
  /\ s' = [s EXCEPT !.lifecycle = "retired", !.coverage = "JoinLane"]

RetiredStutter == /\ s.lifecycle = "retired" /\ UNCHANGED s

Next == CtrlPress \/ BusyCtrlPress \/ OrdinaryPress \/ ModifierChange
     \/ OrdinaryRelease \/ CancelLinkMovement \/ MismatchedLinkRelease \/ ReleaseLink
     \/ ContentChanges \/ InvalidateMap \/ SuccessfulDraw \/ RejectStaleSource
     \/ ResolveNoLink \/ ResolveMatched \/ ResolveNoHandlerHttp
     \/ ResolveNoHandlerOther \/ RegistryDrifts \/ RefuseRegistryDrift
     \/ SubmitPlugin \/ SettlePlugin \/ LaunchBrowser \/ SettleBrowser
     \/ RequestShutdown \/ JoinLane \/ RetiredStutter

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ s.lifecycle \in {"active", "draining", "retired"}
  /\ s.pointer \in PointerOwners /\ s.phase \in Phases /\ s.effect \in Effects
  /\ s.url \in Urls /\ s.handler \in Handlers
  /\ s.contentVersion \in 1..MaxVersion /\ s.paintVersion \in 1..MaxVersion
  /\ s.pressedVersion \in 0..MaxVersion /\ s.sourceVersion \in 0..MaxVersion
  /\ s.job \in 0..MaxClick /\ s.busyRefusals \in 0..MaxClick
  /\ s.pluginAttempts \subseteq 1..MaxClick
  /\ s.registeredSelections \subseteq 1..MaxClick
  /\ s.serverGuarded \subseteq 1..MaxClick
  /\ s.browserAttempts \subseteq 1..MaxClick
  /\ s.pluginEligible \subseteq 1..MaxClick
  /\ s.browserEligible \subseteq 1..MaxClick
  /\ s.staleRejected \subseteq 1..MaxClick
  /\ s.registryRejected \subseteq 1..MaxClick
  /\ s.settled \subseteq 1..MaxClick

OneEffectPerClick == s.pluginAttempts \cap s.browserAttempts = {}
PluginRequiresExactMatch == s.pluginAttempts \subseteq s.pluginEligible
BrowserRequiresDefiniteNoHandler == s.browserAttempts \subseteq s.browserEligible
RegisteredCatalogGuardsEffects ==
  /\ s.pluginAttempts = s.serverGuarded
  /\ (s.pluginAttempts \cup s.browserAttempts) \subseteq s.registeredSelections
StaleSourceHasNoEffect ==
  (s.staleRejected \cap (s.pluginAttempts \cup s.browserAttempts)) = {}
RegistryDriftHasNoEffect ==
  (s.registryRejected \cap (s.pluginAttempts \cup s.browserAttempts)) = {}
LinkNeverForwardsOrAcquires == s.linkForwards = 0 /\ s.controlRequests = 0
RetiredOwnsNoGesture == s.lifecycle = "retired" => s.pointer = "idle"
JoinedLaneHasNoPendingEffect ==
  s.lifecycle = "retired" => s.phase \in {"idle", "settled"}
SubmittedEffectIsFinal ==
  /\ (s.phase = "plugin-submitted" => s.effect = "plugin")
  /\ (s.phase = "browser-submitted" => s.effect = "browser")
=============================================================================
