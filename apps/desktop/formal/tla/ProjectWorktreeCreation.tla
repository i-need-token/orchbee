----------------------- MODULE ProjectWorktreeCreation -----------------------
EXTENDS FiniteSets, TLC

\* Bounded workflow abstraction for one managed Worktree creation per Git
\* repository. Desktop Main owns the durable operation ledger, Git mutation,
\* canonical Workbench document commit, compensation, and restart recovery.
\* Renderer form state, Git command refinement, and filesystem atomicity remain
\* outside this model and are checked by deterministic adapter evidence.

CONSTANT Repositories

Phases == {
  "idle",
  "prepared",
  "git-created",
  "committing",
  "compensating",
  "recovery-required",
  "succeeded"
}

Responses == {"none", "delivered", "failed"}
ActivePhases == {"prepared", "git-created", "committing", "compensating"}
TerminalPhases == {"idle", "recovery-required", "succeeded"}

VARIABLES
  phase,
  gitPresent,
  productPresent,
  ledgerPresent,
  response,
  rendererConnected,
  busyRejected

vars == <<phase, gitPresent, productPresent, ledgerPresent, response,
  rendererConnected, busyRejected>>

Init ==
  /\ phase = [repository \in Repositories |-> "idle"]
  /\ gitPresent = {}
  /\ productPresent = {}
  /\ ledgerPresent = {}
  /\ response = [repository \in Repositories |-> "none"]
  /\ rendererConnected = TRUE
  /\ busyRejected = {}

RequestCreation(repository) ==
  /\ repository \in Repositories
  /\ rendererConnected
  /\ phase[repository] = "idle"
  /\ repository \notin productPresent
  /\ phase' = [phase EXCEPT ![repository] = "prepared"]
  /\ ledgerPresent' = ledgerPresent \cup {repository}
  /\ response' = [response EXCEPT ![repository] = "none"]
  /\ UNCHANGED <<gitPresent, productPresent, rendererConnected, busyRejected>>

RejectBusy(repository) ==
  /\ repository \in Repositories
  /\ rendererConnected
  /\ phase[repository] # "idle"
  /\ repository \notin busyRejected
  /\ busyRejected' = busyRejected \cup {repository}
  /\ UNCHANGED <<phase, gitPresent, productPresent, ledgerPresent, response,
       rendererConnected>>

RetireRenderer ==
  /\ rendererConnected
  /\ rendererConnected' = FALSE
  /\ UNCHANGED <<phase, gitPresent, productPresent, ledgerPresent, response,
       busyRejected>>

ReconnectRenderer ==
  /\ ~rendererConnected
  /\ rendererConnected' = TRUE
  /\ UNCHANGED <<phase, gitPresent, productPresent, ledgerPresent, response,
       busyRejected>>

GitCreationSucceeds(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "prepared"
  /\ repository \notin gitPresent
  /\ gitPresent' = gitPresent \cup {repository}
  /\ UNCHANGED <<phase, productPresent, ledgerPresent, response,
       rendererConnected, busyRejected>>

GitCreationFailsClean(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "prepared"
  /\ repository \notin gitPresent
  /\ phase' = [phase EXCEPT ![repository] = "idle"]
  /\ ledgerPresent' = ledgerPresent \ {repository}
  /\ UNCHANGED <<gitPresent, productPresent, response, rendererConnected,
       busyRejected>>

GitCreationFailsPartial(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "prepared"
  /\ repository \notin gitPresent
  /\ phase' = [phase EXCEPT ![repository] = "compensating"]
  /\ gitPresent' = gitPresent \cup {repository}
  /\ UNCHANGED <<productPresent, ledgerPresent, response, rendererConnected,
       busyRejected>>

PersistGitCreation(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "prepared"
  /\ repository \in gitPresent
  /\ phase' = [phase EXCEPT ![repository] = "git-created"]
  /\ UNCHANGED <<gitPresent, productPresent, ledgerPresent, response,
       rendererConnected, busyRejected>>

RestartBeginCompensation(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "prepared"
  /\ repository \in gitPresent
  /\ repository \notin productPresent
  /\ phase' = [phase EXCEPT ![repository] = "compensating"]
  /\ UNCHANGED <<gitPresent, productPresent, ledgerPresent, response,
       rendererConnected, busyRejected>>

BeginDocumentCommit(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "git-created"
  /\ phase' = [phase EXCEPT ![repository] = "committing"]
  /\ UNCHANGED <<gitPresent, productPresent, ledgerPresent, response,
       rendererConnected, busyRejected>>

CommitDocumentEffect(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "committing"
  /\ repository \notin productPresent
  /\ productPresent' = productPresent \cup {repository}
  /\ UNCHANGED <<phase, gitPresent, ledgerPresent, response,
       rendererConnected, busyRejected>>

DocumentCommitFails(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "committing"
  /\ repository \notin productPresent
  /\ phase' = [phase EXCEPT ![repository] = "compensating"]
  /\ UNCHANGED <<gitPresent, productPresent, ledgerPresent, response,
       rendererConnected, busyRejected>>

FinishDocumentCommit(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "committing"
  /\ repository \in productPresent
  /\ phase' = [phase EXCEPT ![repository] = "succeeded"]
  /\ UNCHANGED <<gitPresent, productPresent, ledgerPresent, response,
       rendererConnected, busyRejected>>

RestartRecoverCommitted(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "committing"
  /\ repository \in productPresent
  /\ phase' = [phase EXCEPT ![repository] = "succeeded"]
  /\ ledgerPresent' = ledgerPresent \ {repository}
  /\ UNCHANGED <<gitPresent, productPresent, response, rendererConnected,
       busyRejected>>

CompensationSucceeds(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "compensating"
  /\ repository \notin productPresent
  /\ phase' = [phase EXCEPT ![repository] = "idle"]
  /\ gitPresent' = gitPresent \ {repository}
  /\ ledgerPresent' = ledgerPresent \ {repository}
  /\ UNCHANGED <<productPresent, response, rendererConnected, busyRejected>>

CompensationFails(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "compensating"
  /\ phase' = [phase EXCEPT ![repository] = "recovery-required"]
  /\ UNCHANGED <<gitPresent, productPresent, ledgerPresent, response,
       rendererConnected, busyRejected>>

RetryRecovery(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "recovery-required"
  /\ phase' = [phase EXCEPT ![repository] = "compensating"]
  /\ UNCHANGED <<gitPresent, productPresent, ledgerPresent, response,
       rendererConnected, busyRejected>>

ClearCommittedLedger(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "succeeded"
  /\ repository \in ledgerPresent
  /\ ledgerPresent' = ledgerPresent \ {repository}
  /\ UNCHANGED <<phase, gitPresent, productPresent, response,
       rendererConnected, busyRejected>>

DeliverSuccess(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "succeeded"
  /\ rendererConnected
  /\ response[repository] = "none"
  /\ response' = [response EXCEPT ![repository] = "delivered"]
  /\ UNCHANGED <<phase, gitPresent, productPresent, ledgerPresent,
       rendererConnected, busyRejected>>

FailSuccessDelivery(repository) ==
  /\ repository \in Repositories
  /\ phase[repository] = "succeeded"
  /\ ~rendererConnected
  /\ response[repository] = "none"
  /\ response' = [response EXCEPT ![repository] = "failed"]
  /\ UNCHANGED <<phase, gitPresent, productPresent, ledgerPresent,
       rendererConnected, busyRejected>>

GitOutcome(repository) ==
  GitCreationSucceeds(repository)
  \/ GitCreationFailsClean(repository)
  \/ GitCreationFailsPartial(repository)

AdvanceCreatedGit(repository) ==
  PersistGitCreation(repository) \/ RestartBeginCompensation(repository)

DocumentOutcome(repository) ==
  CommitDocumentEffect(repository) \/ DocumentCommitFails(repository)

FinishOrRecoverDocumentCommit(repository) ==
  FinishDocumentCommit(repository) \/ RestartRecoverCommitted(repository)

CompensationOutcome(repository) ==
  CompensationSucceeds(repository) \/ CompensationFails(repository)

Next ==
  \/ \E repository \in Repositories: RequestCreation(repository)
  \/ \E repository \in Repositories: RejectBusy(repository)
  \/ RetireRenderer
  \/ ReconnectRenderer
  \/ \E repository \in Repositories: GitOutcome(repository)
  \/ \E repository \in Repositories: AdvanceCreatedGit(repository)
  \/ \E repository \in Repositories: BeginDocumentCommit(repository)
  \/ \E repository \in Repositories: DocumentOutcome(repository)
  \/ \E repository \in Repositories: FinishOrRecoverDocumentCommit(repository)
  \/ \E repository \in Repositories: CompensationOutcome(repository)
  \/ \E repository \in Repositories: RetryRecovery(repository)
  \/ \E repository \in Repositories: ClearCommittedLedger(repository)
  \/ \E repository \in Repositories: DeliverSuccess(repository)
  \/ \E repository \in Repositories: FailSuccessDelivery(repository)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ (\A repository \in Repositories: WF_vars(GitOutcome(repository)))
  /\ (\A repository \in Repositories: WF_vars(AdvanceCreatedGit(repository)))
  /\ (\A repository \in Repositories: WF_vars(BeginDocumentCommit(repository)))
  /\ (\A repository \in Repositories: WF_vars(DocumentOutcome(repository)))
  /\ (\A repository \in Repositories: WF_vars(FinishOrRecoverDocumentCommit(repository)))
  /\ (\A repository \in Repositories: WF_vars(CompensationOutcome(repository)))
  /\ (\A repository \in Repositories: WF_vars(ClearCommittedLedger(repository)))

TypeOK ==
  /\ phase \in [Repositories -> Phases]
  /\ gitPresent \subseteq Repositories
  /\ productPresent \subseteq Repositories
  /\ ledgerPresent \subseteq Repositories
  /\ response \in [Repositories -> Responses]
  /\ rendererConnected \in BOOLEAN
  /\ busyRejected \subseteq Repositories

ProductRequiresGit == productPresent \subseteq gitPresent

SuccessfulOutcomeHasBothOwners ==
  \A repository \in Repositories:
    phase[repository] = "succeeded" =>
      repository \in gitPresent /\ repository \in productPresent

PartialGitAlwaysHasRecoveryLedger ==
  \A repository \in Repositories:
    repository \in gitPresent /\ repository \notin productPresent =>
      repository \in ledgerPresent

PreparedBeforeGitHasNoProductEffect ==
  \A repository \in Repositories:
    phase[repository] = "prepared" /\ repository \notin gitPresent =>
      repository \notin productPresent

IdleUncommittedOperationIsClean ==
  \A repository \in Repositories:
    phase[repository] = "idle" /\ repository \notin productPresent =>
      repository \notin gitPresent /\ repository \notin ledgerPresent

RecoveryNeverRollsBackCommittedProduct ==
  \A repository \in Repositories:
    repository \in productPresent =>
      phase[repository] \in {"committing", "succeeded"}

ResponseOnlyAfterCommittedParity ==
  \A repository \in Repositories:
    response[repository] # "none" =>
      repository \in gitPresent /\ repository \in productPresent

ActiveOperationEventuallySettles ==
  \A repository \in Repositories:
    phase[repository] \in ActivePhases ~> phase[repository] \in TerminalPhases

CommittedLedgerEventuallyClears ==
  \A repository \in Repositories:
    repository \in productPresent /\ repository \in ledgerPresent ~>
      repository \notin ledgerPresent

=============================================================================
