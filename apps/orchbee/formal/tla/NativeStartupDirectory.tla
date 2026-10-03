------------------------ MODULE NativeStartupDirectory ------------------------
EXTENDS Naturals, FiniteSets, TLC

Clients == {1, 2}
Phases == {"Read", "ProjectSubmit", "ProjectWait", "ProjectRead", "WorkspaceSubmit",
           "WorkspaceWait", "WorkspaceRead", "Frame", "Existing", "Failed", "Unknown", "Ready"}
VARIABLES eligible, phase, revision, captured, nonempty, peerWritten, emptyObserved,
          projectOwner, workspaceOwner, issuedProject, issuedWorkspace,
          knownProject, knownWorkspace, started, focus, coverage
vars == <<eligible, phase, revision, captured, nonempty, peerWritten, emptyObserved,
          projectOwner, workspaceOwner, issuedProject, issuedWorkspace,
          knownProject, knownWorkspace, started, focus, coverage>>

Init == /\ eligible \in [Clients -> BOOLEAN]
        /\ phase = [c \in Clients |-> "Read"]
        /\ revision = 0 /\ captured = [c \in Clients |-> 0]
        /\ nonempty = FALSE /\ peerWritten = FALSE /\ emptyObserved = {}
        /\ projectOwner = 0 /\ workspaceOwner = 0
        /\ issuedProject = {} /\ issuedWorkspace = {}
        /\ knownProject = {} /\ knownWorkspace = {} /\ started = {}
        /\ focus = [c \in Clients |-> "None"] /\ coverage = "Init"

PrepareEmpty(c) == /\ phase[c] = "Read" /\ eligible[c] /\ ~nonempty
                   /\ phase' = [phase EXCEPT ![c] = "ProjectSubmit"]
                   /\ captured' = [captured EXCEPT ![c] = revision]
                   /\ emptyObserved' = emptyObserved \cup {c}
                   /\ coverage' = "PrepareEmpty"
                   /\ UNCHANGED <<eligible, revision, nonempty, peerWritten, projectOwner,
                       workspaceOwner, issuedProject, issuedWorkspace, knownProject,
                       knownWorkspace, started, focus>>
SelectExisting(c) == /\ phase[c] = "Read" /\ (~eligible[c] \/ nonempty)
                     /\ phase' = [phase EXCEPT ![c] = "Existing"]
                     /\ focus' = [focus EXCEPT ![c] = "Existing"]
                     /\ coverage' = "SelectExisting"
                     /\ UNCHANGED <<eligible, revision, captured, nonempty, peerWritten,
                         emptyObserved, projectOwner, workspaceOwner, issuedProject,
                         issuedWorkspace, knownProject, knownWorkspace, started>>
SubmitProject(c) == /\ phase[c] = "ProjectSubmit" /\ c \notin issuedProject
                    /\ phase' = [phase EXCEPT ![c] = "ProjectWait"]
                    /\ issuedProject' = issuedProject \cup {c}
                    /\ coverage' = "SubmitProject"
                    /\ UNCHANGED <<eligible, revision, captured, nonempty, peerWritten,
                        emptyObserved, projectOwner, workspaceOwner, issuedWorkspace,
                        knownProject, knownWorkspace, started, focus>>
CommitProject(c) == /\ phase[c] = "ProjectWait" /\ captured[c] = revision /\ ~nonempty
                    /\ phase' = [phase EXCEPT ![c] = "ProjectRead"]
                    /\ revision' = revision + 1 /\ nonempty' = TRUE /\ projectOwner' = c
                    /\ knownProject' = knownProject \cup {c}
                    /\ coverage' = "CommitProject"
                    /\ UNCHANGED <<eligible, captured, peerWritten, emptyObserved,
                        workspaceOwner, issuedProject, issuedWorkspace, knownWorkspace, started, focus>>
Fail(c, at, label) == /\ phase[c] = at
                      /\ phase' = [phase EXCEPT ![c] = "Failed"] /\ coverage' = label
                      /\ UNCHANGED <<eligible, revision, captured, nonempty, peerWritten,
                          emptyObserved, projectOwner, workspaceOwner, issuedProject,
                          issuedWorkspace, knownProject, knownWorkspace, started, focus>>
UnknownProject(c, committed) ==
    /\ phase[c] = "ProjectWait"
    /\ (committed => (captured[c] = revision /\ ~nonempty))
    /\ phase' = [phase EXCEPT ![c] = "Unknown"]
    /\ revision' = IF committed THEN revision + 1 ELSE revision
    /\ nonempty' = (nonempty \/ committed)
    /\ projectOwner' = IF committed THEN c ELSE projectOwner
    /\ coverage' = "UnknownProject"
    /\ UNCHANGED <<eligible, captured, peerWritten, emptyObserved, workspaceOwner,
        issuedProject, issuedWorkspace, knownProject, knownWorkspace, started, focus>>
RefreshProject(c) == /\ phase[c] = "ProjectRead" /\ projectOwner = c
                     /\ phase' = [phase EXCEPT ![c] = "WorkspaceSubmit"]
                     /\ captured' = [captured EXCEPT ![c] = revision]
                     /\ coverage' = "RefreshProject"
                     /\ UNCHANGED <<eligible, revision, nonempty, peerWritten, emptyObserved,
                         projectOwner, workspaceOwner, issuedProject, issuedWorkspace,
                         knownProject, knownWorkspace, started, focus>>
SubmitWorkspace(c) == /\ phase[c] = "WorkspaceSubmit" /\ c \notin issuedWorkspace
                      /\ phase' = [phase EXCEPT ![c] = "WorkspaceWait"]
                      /\ issuedWorkspace' = issuedWorkspace \cup {c}
                      /\ coverage' = "SubmitWorkspace"
                      /\ UNCHANGED <<eligible, revision, captured, nonempty, peerWritten,
                          emptyObserved, projectOwner, workspaceOwner, issuedProject,
                          knownProject, knownWorkspace, started, focus>>
CommitWorkspace(c, outcome) ==
    /\ phase[c] = "WorkspaceWait" /\ captured[c] = revision /\ projectOwner = c
    /\ phase' = [phase EXCEPT ![c] = IF outcome = "Started" THEN "WorkspaceRead" ELSE "Failed"]
    /\ revision' = revision + 1 /\ workspaceOwner' = c
    /\ knownWorkspace' = knownWorkspace \cup {c}
    /\ started' = IF outcome = "Started" THEN started \cup {c} ELSE started
    /\ coverage' = CASE outcome = "Started" -> "CommitStarted"
                     [] outcome = "Pending" -> "CommitPending"
                     [] outcome = "Failed" -> "CommitFailed"
                     [] OTHER -> "CommitUncertain"
    /\ UNCHANGED <<eligible, captured, nonempty, peerWritten, emptyObserved,
        projectOwner, issuedProject, issuedWorkspace, knownProject, focus>>
UnknownWorkspace(c, committed) ==
    /\ phase[c] = "WorkspaceWait"
    /\ (committed => (captured[c] = revision /\ projectOwner = c))
    /\ phase' = [phase EXCEPT ![c] = "Unknown"]
    /\ revision' = IF committed THEN revision + 1 ELSE revision
    /\ workspaceOwner' = IF committed THEN c ELSE workspaceOwner
    /\ coverage' = "UnknownWorkspace"
    /\ UNCHANGED <<eligible, captured, nonempty, peerWritten, emptyObserved,
        projectOwner, issuedProject, issuedWorkspace, knownProject, knownWorkspace, started, focus>>
RefreshWorkspace(c) == /\ phase[c] = "WorkspaceRead" /\ workspaceOwner = c /\ c \in started
                       /\ phase' = [phase EXCEPT ![c] = "Frame"] /\ coverage' = "RefreshWorkspace"
                       /\ UNCHANGED <<eligible, revision, captured, nonempty, peerWritten,
                           emptyObserved, projectOwner, workspaceOwner, issuedProject,
                           issuedWorkspace, knownProject, knownWorkspace, started, focus>>
Draw(c) == /\ phase[c] = "Frame" /\ phase' = [phase EXCEPT ![c] = "Ready"]
           /\ focus' = [focus EXCEPT ![c] = "Created"] /\ coverage' = "Draw"
           /\ UNCHANGED <<eligible, revision, captured, nonempty, peerWritten, emptyObserved,
               projectOwner, workspaceOwner, issuedProject, issuedWorkspace,
               knownProject, knownWorkspace, started>>
PeerWrite == /\ ~peerWritten /\ revision < 3
             /\ peerWritten' = TRUE /\ nonempty' = TRUE /\ revision' = revision + 1
             /\ coverage' = "PeerWrite"
             /\ UNCHANGED <<eligible, phase, captured, emptyObserved, projectOwner,
                 workspaceOwner, issuedProject, issuedWorkspace, knownProject,
                 knownWorkspace, started, focus>>
RejectProject(c) == /\ phase[c] = "ProjectWait" /\ Fail(c, "ProjectWait", "RejectProject")
FailProjectRead(c) == /\ phase[c] = "ProjectRead" /\ Fail(c, "ProjectRead", "FailProjectRead")
RejectWorkspace(c) == /\ phase[c] = "WorkspaceWait" /\ Fail(c, "WorkspaceWait", "RejectWorkspace")
FailWorkspaceRead(c) == /\ phase[c] = "WorkspaceRead" /\ Fail(c, "WorkspaceRead", "FailWorkspaceRead")
FailDraw(c) == /\ phase[c] = "Frame" /\ Fail(c, "Frame", "FailDraw")
CommitStarted(c) == /\ phase[c] = "WorkspaceWait" /\ CommitWorkspace(c, "Started")
CommitPending(c) == /\ phase[c] = "WorkspaceWait" /\ CommitWorkspace(c, "Pending")
CommitFailed(c) == /\ phase[c] = "WorkspaceWait" /\ CommitWorkspace(c, "Failed")
CommitUncertain(c) == /\ phase[c] = "WorkspaceWait" /\ CommitWorkspace(c, "Uncertain")
Next == PeerWrite \/ \E c \in Clients:
    PrepareEmpty(c) \/ SelectExisting(c) \/ SubmitProject(c) \/ CommitProject(c)
    \/ RejectProject(c) \/ UnknownProject(c, TRUE) \/ UnknownProject(c, FALSE)
    \/ RefreshProject(c) \/ FailProjectRead(c) \/ SubmitWorkspace(c)
    \/ CommitStarted(c) \/ CommitPending(c) \/ CommitFailed(c) \/ CommitUncertain(c)
    \/ RejectWorkspace(c) \/ UnknownWorkspace(c, TRUE) \/ UnknownWorkspace(c, FALSE)
    \/ RefreshWorkspace(c) \/ FailWorkspaceRead(c) \/ Draw(c) \/ FailDraw(c)
Spec == Init /\ [][Next]_vars
TypeOK == /\ eligible \in [Clients -> BOOLEAN] /\ phase \in [Clients -> Phases]
          /\ revision \in 0..3 /\ captured \in [Clients -> 0..3]
          /\ nonempty \in BOOLEAN /\ peerWritten \in BOOLEAN
          /\ projectOwner \in Clients \cup {0} /\ workspaceOwner \in Clients \cup {0}
          /\ emptyObserved \subseteq Clients /\ issuedProject \subseteq Clients
          /\ issuedWorkspace \subseteq Clients /\ knownProject \subseteq Clients
          /\ knownWorkspace \subseteq Clients /\ started \subseteq Clients
          /\ focus \in [Clients -> {"None", "Existing", "Created"}]
EmptyAdmission == issuedProject \subseteq emptyObserved
WorkspaceRequiresKnownProject == issuedWorkspace \subseteq knownProject \cap issuedProject
NoFalseReady == \A c \in Clients: focus[c] = "Created" => c \in started \cap knownWorkspace
KnownReceiptsPreserved == [][knownProject \subseteq knownProject' /\ knownWorkspace \subseteq knownWorkspace']_vars
IndependentFocus == [][Cardinality({c \in Clients: focus[c] # focus'[c]}) <= 1]_vars
=============================================================================
