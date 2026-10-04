---------------- MODULE WindowsConfigurationPublication ----------------
EXTENDS Naturals, FiniteSets, TLC

\* The shared Windows settings owner admits one complete effective-settings and
\* diagnostics pair. Fatal candidates never publish. Accepted candidates publish
\* one immutable snapshot. A Client may begin with a provisional local projection,
\* applies its first complete authoritative snapshot once (including revision zero), then
\* only applies newer snapshots.
\* Delivery fairness is intentionally absent; connection tests own propagation.

CONSTANTS MaxRevision, MaxWarnings, MaxOmitted
ASSUME MaxRevision = 3 /\ MaxWarnings = 2 /\ MaxOmitted = 2

Settings == {"baseline", "alternate"}
Diagnostics == [count: 0..MaxWarnings, omitted: 0..MaxOmitted]
CleanDiagnostics == [count |-> 0, omitted |-> 0]
WarningDiagnostics == Diagnostics \ {CleanDiagnostics}
Snapshots ==
  [revision: 0..MaxRevision, settings: Settings, diagnostics: Diagnostics]

VARIABLES serverRevision, serverSettings, serverDiagnostics, published,
          clientInitialized, clientRevision, clientSettings, clientDiagnostics
vars == <<serverRevision, serverSettings, serverDiagnostics, published,
          clientInitialized, clientRevision, clientSettings, clientDiagnostics>>

ServerSnapshot ==
  [revision |-> serverRevision, settings |-> serverSettings,
   diagnostics |-> serverDiagnostics]

ClientSnapshot ==
  [revision |-> clientRevision, settings |-> clientSettings,
   diagnostics |-> clientDiagnostics]

Init ==
  /\ serverRevision = 0
  /\ serverSettings \in Settings
  /\ serverDiagnostics \in Diagnostics
  /\ published = {ServerSnapshot}
  /\ clientInitialized = FALSE
  /\ clientRevision = 0
  /\ clientSettings \in Settings
  /\ clientDiagnostics \in Diagnostics

Publish(nextSettings, nextDiagnostics) ==
  /\ serverRevision < MaxRevision
  /\ \/ serverRevision = 0
     \/ <<nextSettings, nextDiagnostics>> #
          <<serverSettings, serverDiagnostics>>
  /\ serverRevision' = serverRevision + 1
  /\ serverSettings' = nextSettings
  /\ serverDiagnostics' = nextDiagnostics
  /\ published' = published \cup {
       [revision |-> serverRevision + 1, settings |-> nextSettings,
        diagnostics |-> nextDiagnostics]}
  /\ UNCHANGED <<clientInitialized, clientRevision, clientSettings, clientDiagnostics>>

ReloadWarning ==
  /\ serverRevision < MaxRevision
  /\ \E nextSettings \in Settings, nextDiagnostics \in WarningDiagnostics:
       Publish(nextSettings, nextDiagnostics)

ReloadClean ==
  /\ serverRevision < MaxRevision
  /\ \E nextSettings \in Settings:
       Publish(nextSettings, CleanDiagnostics)

PublishWarningWithoutSettingChange ==
  \E nextDiagnostics \in WarningDiagnostics:
    /\ nextDiagnostics # serverDiagnostics
    /\ Publish(serverSettings, nextDiagnostics)

ClearWarningWithoutSettingChange ==
  /\ serverDiagnostics # CleanDiagnostics
  /\ Publish(serverSettings, CleanDiagnostics)

PublishFirstUnchanged ==
  /\ serverRevision = 0
  /\ Publish(serverSettings, serverDiagnostics)

\* A fatal read/TOML/environment candidate has no admitted snapshot and cannot
\* alter settings, diagnostics, revision, or the set available to Clients.
ReloadFatal == UNCHANGED vars

ApplyInitialSnapshot ==
  \E snapshot \in published:
    /\ ~clientInitialized
    /\ clientInitialized' = TRUE
    /\ clientRevision' = snapshot.revision
    /\ clientSettings' = snapshot.settings
    /\ clientDiagnostics' = snapshot.diagnostics
    /\ UNCHANGED <<serverRevision, serverSettings, serverDiagnostics, published>>

ApplyNewerSnapshot ==
  \E snapshot \in published:
    /\ clientInitialized
    /\ snapshot.revision > clientRevision
    /\ clientInitialized' = TRUE
    /\ clientRevision' = snapshot.revision
    /\ clientSettings' = snapshot.settings
    /\ clientDiagnostics' = snapshot.diagnostics
    /\ UNCHANGED <<serverRevision, serverSettings, serverDiagnostics, published>>

RejectStaleSnapshot ==
  /\ clientInitialized
  /\ \E snapshot \in published: snapshot.revision <= clientRevision
  /\ UNCHANGED vars

Next ==
  \/ ReloadWarning
  \/ ReloadClean
  \/ PublishWarningWithoutSettingChange
  \/ ClearWarningWithoutSettingChange
  \/ PublishFirstUnchanged
  \/ ReloadFatal
  \/ ApplyInitialSnapshot
  \/ ApplyNewerSnapshot
  \/ RejectStaleSnapshot

Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ serverRevision \in 0..MaxRevision
  /\ serverSettings \in Settings
  /\ serverDiagnostics \in Diagnostics
  /\ published \subseteq Snapshots
  /\ clientInitialized \in BOOLEAN
  /\ clientRevision \in 0..MaxRevision
  /\ clientSettings \in Settings
  /\ clientDiagnostics \in Diagnostics

OneSnapshotPerRevision ==
  \A revision \in 0..MaxRevision:
    Cardinality({snapshot \in published: snapshot.revision = revision}) <= 1

ServerGenerationIsPublished == ServerSnapshot \in published

ClientGenerationIsPublished ==
  \/ ~clientInitialized
  \/ ClientSnapshot \in published

ClientDoesNotLeadServer == clientRevision <= serverRevision

DiagnosticsAreBounded ==
  /\ serverDiagnostics.count <= MaxWarnings
  /\ serverDiagnostics.omitted <= MaxOmitted
  /\ clientDiagnostics.count <= MaxWarnings
  /\ clientDiagnostics.omitted <= MaxOmitted

=============================================================================
