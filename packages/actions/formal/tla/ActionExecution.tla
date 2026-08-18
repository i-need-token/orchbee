--------------------------- MODULE ActionExecution ---------------------------
EXTENDS FiniteSets, Naturals, Sequences, TLC

\* Refines the ActionRuntime contract for one semantic concurrency key. Three
\* executions expose a cancelled serialized middle entry and a later successor.
\* Registration generations model prepared-token invalidation; already-bound
\* executions retain their exact action and invocation snapshot across disposal
\* or replacement. Caller context mutation is intentionally independent.

CONSTANT InitialPolicy, ExecutionCount, ExerciseRegistrationLifecycle,
  ExerciseHandlerLifecycle

Executions == 1..ExecutionCount
Contexts == {"context-a", "context-b"}
NoContext == "no-context"
NoGeneration == 0
MaxGeneration == 2
Policies == {"parallel", "reject-duplicate", "replace-latest", "serialize"}
ASSUME
  /\ InitialPolicy \in Policies
  /\ ExecutionCount \in {1, 2, 3}
  /\ ExerciseRegistrationLifecycle \in BOOLEAN
  /\ ExerciseHandlerLifecycle \in BOOLEAN
NonTerminal == {"absent", "prepared", "resolving", "queued", "awaiting", "running", "cancelling"}
Terminal == {"completed", "unchanged", "blocked", "cancelled", "failed", "unsupported"}
States == NonTerminal \cup Terminal
Actions == {
  "init", "mutate-context", "prepare", "invoke", "invoke-unsupported",
  "invoke-after-dispose", "execute-prepared", "execute-prepared-stale",
  "execute-prepared-after-dispose", "dispose-registration",
  "replace-registration", "resolve-parallel", "resolve-reject-first",
  "reject-duplicate", "resolve-replacement-first",
  "resolve-replacement-after", "resolve-serialized", "start-queued",
  "accept-confirmation", "reject-confirmation", "report-progress", "commit",
  "cancel-before-handler", "cancel-running", "finish", "dispose-runtime"
}
NoExecution == 0

VARIABLES
  policy,
  runtimeActive,
  registrationActive,
  registrationGeneration,
  callerContext,
  phase,
  bindingGeneration,
  capturedContext,
  handlerContext,
  committed,
  handlerStarts,
  resultCount,
  progressCount,
  invocationOrder,
  currentExecution,
  stalePrepared,
  lastAction,
  lastTarget

vars == <<policy, runtimeActive, registrationActive, registrationGeneration,
  callerContext, phase, bindingGeneration, capturedContext, handlerContext,
  committed, handlerStarts, resultCount, progressCount, invocationOrder,
  currentExecution, stalePrepared, lastAction, lastTarget>>

Range(sequence) == {sequence[index] : index \in DOMAIN sequence}

IndexOf(sequence, value) ==
  CHOOSE index \in 1..Len(sequence): sequence[index] = value

EarlierExecutions(execution) ==
  IF execution \in Range(invocationOrder)
  THEN {invocationOrder[index] : index \in 1..(IndexOf(invocationOrder, execution) - 1)}
  ELSE {}

IsScheduled(execution) ==
  phase[execution] \in {"resolving", "queued", "awaiting", "running", "cancelling"}

IsActive(execution) == phase[execution] \in {"awaiting", "running", "cancelling"}

ActiveExecutions == {execution \in Executions : IsActive(execution)}

FinishOutcomes ==
  IF ExerciseHandlerLifecycle
  THEN {"completed", "unchanged", "cancelled", "failed"}
  ELSE {"completed"}

HasEarlierNonterminal(execution) ==
  \E earlier \in EarlierExecutions(execution): phase[earlier] \notin Terminal

CanStartQueued(execution) ==
  /\ runtimeActive
  /\ policy = "serialize"
  /\ phase[execution] = "queued"
  /\ currentExecution = NoExecution
  /\ \A earlier \in EarlierExecutions(execution): phase[earlier] \in Terminal

StartableQueued == {execution \in Executions : CanStartQueued(execution)}

Record(action, execution) ==
  /\ lastAction' = action
  /\ lastTarget' = execution

Init ==
  /\ policy = InitialPolicy
  /\ runtimeActive = TRUE
  /\ registrationActive = TRUE
  /\ registrationGeneration = 1
  /\ callerContext = "context-a"
  /\ phase = [execution \in Executions |-> "absent"]
  /\ bindingGeneration = [execution \in Executions |-> NoGeneration]
  /\ capturedContext = [execution \in Executions |-> NoContext]
  /\ handlerContext = [execution \in Executions |-> NoContext]
  /\ committed = {}
  /\ handlerStarts = [execution \in Executions |-> 0]
  /\ resultCount = [execution \in Executions |-> 0]
  /\ progressCount = [execution \in Executions |-> 0]
  /\ invocationOrder = <<>>
  /\ currentExecution = NoExecution
  /\ stalePrepared = {}
  /\ lastAction = "init"
  /\ lastTarget = NoExecution

MutateCallerContext(nextContext) ==
  /\ ExerciseHandlerLifecycle
  /\ nextContext \in Contexts \ {callerContext}
  /\ callerContext' = nextContext
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, phase, bindingGeneration, capturedContext,
       handlerContext, committed, handlerStarts, resultCount, progressCount,
       invocationOrder, currentExecution, stalePrepared>>
  /\ Record("mutate-context", NoExecution)

Prepare(execution) ==
  /\ ExerciseRegistrationLifecycle
  /\ runtimeActive
  /\ registrationActive
  /\ phase[execution] = "absent"
  /\ phase' = [phase EXCEPT ![execution] = "prepared"]
  /\ bindingGeneration' = [bindingGeneration EXCEPT ![execution] = registrationGeneration]
  /\ capturedContext' = [capturedContext EXCEPT ![execution] = callerContext]
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, handlerContext, committed,
       handlerStarts, resultCount, progressCount, invocationOrder,
       currentExecution, stalePrepared>>
  /\ Record("prepare", execution)

Invoke(execution) ==
  /\ runtimeActive
  /\ registrationActive
  /\ phase[execution] = "absent"
  /\ phase' = [phase EXCEPT ![execution] = "resolving"]
  /\ bindingGeneration' = [bindingGeneration EXCEPT ![execution] = registrationGeneration]
  /\ capturedContext' = [capturedContext EXCEPT ![execution] = callerContext]
  /\ invocationOrder' = Append(invocationOrder, execution)
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, handlerContext, committed,
       handlerStarts, resultCount, progressCount, currentExecution, stalePrepared>>
  /\ Record("invoke", execution)

InvokeUnsupported(execution) ==
  /\ runtimeActive
  /\ ~registrationActive
  /\ phase[execution] = "absent"
  /\ phase' = [phase EXCEPT ![execution] = "unsupported"]
  /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts,
       progressCount, invocationOrder, currentExecution, stalePrepared>>
  /\ Record("invoke-unsupported", execution)

InvokeAfterDispose(execution) ==
  /\ ~runtimeActive
  /\ phase[execution] = "absent"
  /\ phase' = [phase EXCEPT ![execution] = "cancelled"]
  /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts,
       progressCount, invocationOrder, currentExecution, stalePrepared>>
  /\ Record("invoke-after-dispose", execution)

ExecutePrepared(execution) ==
  /\ runtimeActive
  /\ registrationActive
  /\ phase[execution] = "prepared"
  /\ bindingGeneration[execution] = registrationGeneration
  /\ phase' = [phase EXCEPT ![execution] = "resolving"]
  /\ invocationOrder' = Append(invocationOrder, execution)
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts, resultCount,
       progressCount, currentExecution, stalePrepared>>
  /\ Record("execute-prepared", execution)

ExecuteStalePrepared(execution) ==
  /\ runtimeActive
  /\ phase[execution] = "prepared"
  /\ (~registrationActive \/ bindingGeneration[execution] # registrationGeneration)
  /\ phase' = [phase EXCEPT ![execution] = "blocked"]
  /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ stalePrepared' = stalePrepared \cup {execution}
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts,
       progressCount, invocationOrder, currentExecution>>
  /\ Record("execute-prepared-stale", execution)

ExecutePreparedAfterDispose(execution) ==
  /\ ~runtimeActive
  /\ phase[execution] = "prepared"
  /\ phase' = [phase EXCEPT ![execution] = "cancelled"]
  /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts,
       progressCount, invocationOrder, currentExecution, stalePrepared>>
  /\ Record("execute-prepared-after-dispose", execution)

DisposeRegistration ==
  /\ ExerciseRegistrationLifecycle
  /\ runtimeActive
  /\ registrationActive
  /\ registrationActive' = FALSE
  /\ UNCHANGED <<policy, runtimeActive, registrationGeneration, callerContext,
       phase, bindingGeneration, capturedContext, handlerContext, committed,
       handlerStarts, resultCount, progressCount, invocationOrder,
       currentExecution, stalePrepared>>
  /\ Record("dispose-registration", NoExecution)

ReplaceRegistration ==
  /\ ExerciseRegistrationLifecycle
  /\ runtimeActive
  /\ ~registrationActive
  /\ registrationGeneration < MaxGeneration
  /\ registrationActive' = TRUE
  /\ registrationGeneration' = registrationGeneration + 1
  /\ UNCHANGED <<policy, runtimeActive, callerContext, phase,
       bindingGeneration, capturedContext, handlerContext, committed,
       handlerStarts, resultCount, progressCount, invocationOrder,
       currentExecution, stalePrepared>>
  /\ Record("replace-registration", NoExecution)

ResolveParallel(execution) ==
  /\ policy = "parallel"
  /\ phase[execution] = "resolving"
  /\ phase' = [phase EXCEPT ![execution] = "awaiting"]
  /\ UNCHANGED <<policy, runtimeActive, registrationActive, registrationGeneration,
       callerContext, bindingGeneration, capturedContext, handlerContext,
       committed, handlerStarts, resultCount, progressCount, invocationOrder,
       currentExecution, stalePrepared>>
  /\ Record("resolve-parallel", execution)

ResolveRejectFirst(execution) ==
  /\ policy = "reject-duplicate"
  /\ phase[execution] = "resolving"
  /\ currentExecution = NoExecution
  /\ phase' = [phase EXCEPT ![execution] = "awaiting"]
  /\ currentExecution' = execution
  /\ UNCHANGED <<policy, runtimeActive, registrationActive, registrationGeneration,
       callerContext, bindingGeneration, capturedContext, handlerContext,
       committed, handlerStarts, resultCount, progressCount, invocationOrder,
       stalePrepared>>
  /\ Record("resolve-reject-first", execution)

RejectDuplicate(execution) ==
  /\ policy = "reject-duplicate"
  /\ phase[execution] = "resolving"
  /\ currentExecution # NoExecution
  /\ phase' = [phase EXCEPT ![execution] = "blocked"]
  /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ UNCHANGED <<policy, runtimeActive, registrationActive, registrationGeneration,
       callerContext, bindingGeneration, capturedContext, handlerContext,
       committed, handlerStarts, progressCount, invocationOrder,
       currentExecution, stalePrepared>>
  /\ Record("reject-duplicate", execution)

ResolveReplacementFirst(execution) ==
  /\ policy = "replace-latest"
  /\ phase[execution] = "resolving"
  /\ currentExecution = NoExecution
  /\ phase' = [phase EXCEPT ![execution] = "awaiting"]
  /\ currentExecution' = execution
  /\ UNCHANGED <<policy, runtimeActive, registrationActive, registrationGeneration,
       callerContext, bindingGeneration, capturedContext, handlerContext,
       committed, handlerStarts, resultCount, progressCount, invocationOrder,
       stalePrepared>>
  /\ Record("resolve-replacement-first", execution)

ResolveReplacementAfter(execution, previous) ==
  /\ policy = "replace-latest"
  /\ phase[execution] = "resolving"
  /\ previous = currentExecution
  /\ previous # NoExecution
  /\ phase' = [phase EXCEPT
       ![execution] = "awaiting",
       ![previous] = IF phase[previous] = "awaiting" THEN "cancelled" ELSE "cancelling"]
  /\ resultCount' = IF phase[previous] = "awaiting"
       THEN [resultCount EXCEPT ![previous] = @ + 1]
       ELSE resultCount
  /\ currentExecution' = execution
  /\ UNCHANGED <<policy, runtimeActive, registrationActive, registrationGeneration,
       callerContext, bindingGeneration, capturedContext, handlerContext,
       committed, handlerStarts, progressCount, invocationOrder, stalePrepared>>
  /\ Record("resolve-replacement-after", execution)

ResolveSerialized(execution) ==
  /\ policy = "serialize"
  /\ phase[execution] = "resolving"
  /\ phase' = [phase EXCEPT
       ![execution] = IF HasEarlierNonterminal(execution) \/ currentExecution # NoExecution
         THEN "queued"
         ELSE "awaiting"]
  /\ currentExecution' =
       IF HasEarlierNonterminal(execution) \/ currentExecution # NoExecution
       THEN currentExecution
       ELSE execution
  /\ UNCHANGED <<policy, runtimeActive, registrationActive, registrationGeneration,
       callerContext, bindingGeneration, capturedContext, handlerContext,
       committed, handlerStarts, resultCount, progressCount, invocationOrder,
       stalePrepared>>
  /\ Record("resolve-serialized", execution)

StartQueued(execution) ==
  /\ CanStartQueued(execution)
  /\ phase' = [phase EXCEPT ![execution] = "awaiting"]
  /\ currentExecution' = execution
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts, resultCount,
       progressCount, invocationOrder, stalePrepared>>
  /\ Record("start-queued", execution)

AcceptConfirmation(execution) ==
  /\ phase[execution] = "awaiting"
  /\ phase' = [phase EXCEPT ![execution] = "running"]
  /\ handlerStarts' = [handlerStarts EXCEPT ![execution] = @ + 1]
  /\ handlerContext' = [handlerContext EXCEPT ![execution] = capturedContext[execution]]
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, committed, resultCount, progressCount, invocationOrder,
       currentExecution, stalePrepared>>
  /\ Record("accept-confirmation", execution)

RejectConfirmation(execution) ==
  /\ ExerciseHandlerLifecycle
  /\ phase[execution] = "awaiting"
  /\ phase' = [phase EXCEPT ![execution] = "cancelled"]
  /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ currentExecution' =
       IF currentExecution = execution THEN NoExecution ELSE currentExecution
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts,
       progressCount, invocationOrder, stalePrepared>>
  /\ Record("reject-confirmation", execution)

ReportProgress(execution) ==
  /\ ExerciseHandlerLifecycle
  /\ phase[execution] \in {"running", "cancelling"}
  /\ progressCount[execution] = 0
  /\ progressCount' = [progressCount EXCEPT ![execution] = 1]
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, phase, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts, resultCount,
       invocationOrder, currentExecution, stalePrepared>>
  /\ Record("report-progress", execution)

Commit(execution) ==
  /\ ExerciseHandlerLifecycle
  /\ phase[execution] \in {"running", "cancelling"}
  /\ execution \notin committed
  /\ committed' = committed \cup {execution}
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, phase, bindingGeneration,
       capturedContext, handlerContext, handlerStarts, resultCount,
       progressCount, invocationOrder, currentExecution, stalePrepared>>
  /\ Record("commit", execution)

CancelBeforeHandler(execution) ==
  /\ phase[execution] \in {"resolving", "queued", "awaiting"}
  /\ phase' = [phase EXCEPT ![execution] = "cancelled"]
  /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ currentExecution' =
       IF currentExecution = execution THEN NoExecution ELSE currentExecution
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts,
       progressCount, invocationOrder, stalePrepared>>
  /\ Record("cancel-before-handler", execution)

CancelRunning(execution) ==
  /\ ExerciseHandlerLifecycle
  /\ phase[execution] = "running"
  /\ phase' = [phase EXCEPT ![execution] = "cancelling"]
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts, resultCount,
       progressCount, invocationOrder, currentExecution, stalePrepared>>
  /\ Record("cancel-running", execution)

Finish(execution, outcome) ==
  /\ phase[execution] \in {"running", "cancelling"}
  /\ outcome \in {"completed", "unchanged", "cancelled", "failed"}
  /\ phase' = [phase EXCEPT
       ![execution] =
         IF phase[execution] = "cancelling" /\ execution \notin committed
         THEN "cancelled"
         ELSE IF outcome = "cancelled" /\ execution \in committed
         THEN "failed"
         ELSE outcome]
  /\ resultCount' = [resultCount EXCEPT ![execution] = @ + 1]
  /\ currentExecution' =
       IF currentExecution = execution THEN NoExecution ELSE currentExecution
  /\ UNCHANGED <<policy, runtimeActive, registrationActive,
       registrationGeneration, callerContext, bindingGeneration,
       capturedContext, handlerContext, committed, handlerStarts,
       progressCount, invocationOrder, stalePrepared>>
  /\ Record("finish", execution)

DisposeRuntime ==
  /\ runtimeActive
  /\ runtimeActive' = FALSE
  /\ registrationActive' = FALSE
  /\ currentExecution' = NoExecution
  /\ phase' = [execution \in Executions |->
       IF phase[execution] \in {"resolving", "queued", "awaiting"}
       THEN "cancelled"
       ELSE IF phase[execution] = "running"
       THEN "cancelling"
       ELSE phase[execution]]
  /\ resultCount' = [execution \in Executions |->
       IF phase[execution] \in {"resolving", "queued", "awaiting"}
       THEN resultCount[execution] + 1
       ELSE resultCount[execution]]
  /\ UNCHANGED <<policy, registrationGeneration, callerContext,
       bindingGeneration, capturedContext, handlerContext, committed,
       handlerStarts, progressCount, invocationOrder, stalePrepared>>
  /\ Record("dispose-runtime", NoExecution)

StartAnyQueued == \E execution \in Executions: StartQueued(execution)

Next ==
  \/ \E context \in Contexts: MutateCallerContext(context)
  \/ \E execution \in Executions: Prepare(execution)
  \/ \E execution \in Executions: Invoke(execution)
  \/ \E execution \in Executions: InvokeUnsupported(execution)
  \/ \E execution \in Executions: InvokeAfterDispose(execution)
  \/ \E execution \in Executions: ExecutePrepared(execution)
  \/ \E execution \in Executions: ExecuteStalePrepared(execution)
  \/ \E execution \in Executions: ExecutePreparedAfterDispose(execution)
  \/ DisposeRegistration
  \/ ReplaceRegistration
  \/ \E execution \in Executions: ResolveParallel(execution)
  \/ \E execution \in Executions: ResolveRejectFirst(execution)
  \/ \E execution \in Executions: RejectDuplicate(execution)
  \/ \E execution \in Executions: ResolveReplacementFirst(execution)
  \/ \E execution, previous \in Executions:
       ResolveReplacementAfter(execution, previous)
  \/ \E execution \in Executions: ResolveSerialized(execution)
  \/ StartAnyQueued
  \/ \E execution \in Executions: AcceptConfirmation(execution)
  \/ \E execution \in Executions: RejectConfirmation(execution)
  \/ \E execution \in Executions: ReportProgress(execution)
  \/ \E execution \in Executions: Commit(execution)
  \/ \E execution \in Executions: CancelBeforeHandler(execution)
  \/ \E execution \in Executions: CancelRunning(execution)
  \/ \E execution \in Executions,
       outcome \in FinishOutcomes:
       Finish(execution, outcome)
  \/ DisposeRuntime

Spec == Init /\ [][Next]_vars /\ WF_vars(StartAnyQueued)

TypeOK ==
  /\ policy \in Policies
  /\ runtimeActive \in BOOLEAN
  /\ registrationActive \in BOOLEAN
  /\ registrationGeneration \in 1..MaxGeneration
  /\ callerContext \in Contexts
  /\ phase \in [Executions -> States]
  /\ bindingGeneration \in [Executions -> 0..MaxGeneration]
  /\ capturedContext \in [Executions -> Contexts \cup {NoContext}]
  /\ handlerContext \in [Executions -> Contexts \cup {NoContext}]
  /\ committed \subseteq Executions
  /\ handlerStarts \in [Executions -> 0..1]
  /\ resultCount \in [Executions -> 0..1]
  /\ progressCount \in [Executions -> 0..1]
  /\ invocationOrder \in Seq(Executions)
  /\ Len(invocationOrder) = Cardinality(Range(invocationOrder))
  /\ currentExecution \in Executions \cup {NoExecution}
  /\ stalePrepared \subseteq Executions
  /\ lastAction \in Actions
  /\ lastTarget \in Executions \cup {NoExecution}

CurrentExecutionMatchesEntry ==
  /\ (policy = "parallel") => currentExecution = NoExecution
  /\ currentExecution # NoExecution => IsActive(currentExecution)

RegistrationLifecycleIsolation ==
  \/ ExerciseRegistrationLifecycle
  \/ /\ registrationActive = runtimeActive
     /\ registrationGeneration = 1
     /\ stalePrepared = {}

HandlerLifecycleIsolation ==
  \/ ExerciseHandlerLifecycle
  \/ /\ callerContext = "context-a"
     /\ committed = {}
     /\ \A execution \in Executions: progressCount[execution] = 0

ExactlyOneTerminalResult ==
  \A execution \in Executions:
    (phase[execution] \in Terminal) = (resultCount[execution] = 1)

AtMostOneHandler ==
  \A execution \in Executions: handlerStarts[execution] <= 1

HandlerUsesCapturedContext ==
  \A execution \in Executions:
    handlerStarts[execution] = 1 =>
      /\ capturedContext[execution] \in Contexts
      /\ handlerContext[execution] = capturedContext[execution]

BoundExecutionKeepsRegistrationGeneration ==
  \A execution \in Executions:
    phase[execution] \in {"prepared", "resolving", "queued", "awaiting", "running", "cancelling"} =>
      bindingGeneration[execution] \in 1..MaxGeneration

StalePreparedNeverStarts ==
  \A execution \in stalePrepared:
    /\ phase[execution] = "blocked"
    /\ handlerStarts[execution] = 0
    /\ resultCount[execution] = 1

ProgressRequiresHandler ==
  \A execution \in Executions:
    progressCount[execution] > 0 => handlerStarts[execution] = 1

CommittedExecutionIsNeverCancelled ==
  \A execution \in committed: phase[execution] # "cancelled"

CancellationBeforeCommitDoesNotComplete ==
  \A execution \in Executions:
    phase[execution] = "completed" =>
      execution \in committed \/ handlerStarts[execution] = 1

SerializedExclusion ==
  policy = "serialize" => Cardinality(ActiveExecutions) <= 1

RejectDuplicateExclusion ==
  policy = "reject-duplicate" => Cardinality(ActiveExecutions) <= 1

ReplacementHasOneCurrentHandler ==
  policy = "replace-latest" =>
    Cardinality({execution \in Executions:
      phase[execution] \in {"awaiting", "running"}}) <= 1

SerializedInvocationOrder ==
  policy = "serialize" =>
    \A execution \in ActiveExecutions:
      \A earlier \in EarlierExecutions(execution): phase[earlier] \in Terminal

RuntimeDisposalClosesAdmission ==
  ~runtimeActive =>
    /\ ~registrationActive
    /\ \A execution \in Executions:
         phase[execution] \notin {"resolving", "queued", "awaiting", "running"}

SerializedReadyQueueEventuallyAdvances ==
  []((runtimeActive /\ policy = "serialize" /\ ActiveExecutions = {} /\ StartableQueued # {})
     => <> (ActiveExecutions # {} \/ StartableQueued = {}))

=============================================================================
