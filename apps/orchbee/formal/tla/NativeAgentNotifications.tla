---------------------- MODULE NativeAgentNotifications ----------------------
EXTENDS Naturals, FiniteSets, TLC

\* Owned by the automatic physical-Agent contract in windows-notifications.md
\* and the acknowledgement boundary in native-client-attention.md. The model
\* composes independent Client coordinates while observations, successful
\* paints, time, configuration publication and sink availability may arrive
\* in any order. The inventory configuration exhausts one symmetric coordinate;
\* the contract documents its projection to every finite Client composition.

CONSTANTS Clients, MaxTime, MaxVersion, MaxGeneration, MaxDraw,
          MaxConfigRevision
ASSUME IsFiniteSet(Clients) /\ Clients # {}
       /\ MaxTime = 3 /\ MaxVersion = 3 /\ MaxGeneration = 1
       /\ MaxDraw = 3 /\ MaxConfigRevision = 1

States == {"idle", "working", "blocked", "unknown"}
Kinds == {"done", "request"}
NoState == "none"

NoCandidate ==
  [kind |-> "none", generation |-> 0, version |-> 0,
   from |-> NoState, to |-> NoState, due |-> 0, draw |-> 0, config |-> 0]

CandidateRecords ==
  [kind: Kinds, generation: 0..MaxGeneration, version: 1..MaxVersion,
   from: States, to: States, due: 0..MaxTime, draw: 1..MaxDraw,
   config: 0..MaxConfigRevision]

NoDeliveryPosition == [generation |-> 0, version |-> 0]
DeliveryPositions ==
  [generation: 0..MaxGeneration, version: 1..MaxVersion]

\* Same-generation versions only advance; a replacement advances generation
\* before resetting version, and an observation gap preserves both. Therefore
\* a legal Client emission position strictly increases and the last position
\* is a sufficient witness for any old-key reuse.
ViolationKinds ==
  {"bad-trigger", "seen-or-busy", "muted-sound", "disabled-policy",
   "position-reuse"}

DeliveryPosition(effect) ==
  [generation |-> effect.generation, version |-> effect.version]

PositionStrictlyAfter(next, previous) ==
  \/ previous = NoDeliveryPosition
  \/ next.generation > previous.generation
  \/ next.generation = previous.generation /\ next.version > previous.version

AllowedDeliveryTrigger(effect) ==
  \/ effect.kind = "done" /\ effect.from = "working" /\ effect.to = "idle"
  \/ effect.kind = "request" /\ effect.from \in {"idle", "working"}
     /\ effect.to = "blocked"

DeliveryViolations(effect, previous) ==
  (IF AllowedDeliveryTrigger(effect) THEN {} ELSE {"bad-trigger"})
  \cup (IF ~effect.seenAtDelivery /\ ~effect.sinkBusyAtDelivery
        THEN {} ELSE {"seen-or-busy"})
  \cup (IF ~effect.muted \/ ~effect.sound THEN {} ELSE {"muted-sound"})
  \cup (IF effect.popup \/ effect.sound THEN {} ELSE {"disabled-policy"})
  \cup (IF PositionStrictlyAfter(DeliveryPosition(effect), previous)
        THEN {} ELSE {"position-reuse"})

VARIABLES status, available, observed, version, generation, seen, candidate,
          configRevision, popup, sound, muted, sinkBusy, now, draw,
          lastEmitted, violations
vars == <<status, available, observed, version, generation, seen, candidate,
          configRevision, popup, sound, muted, sinkBusy, now, draw,
          lastEmitted, violations>>

Init ==
  /\ status = [c \in Clients |-> "live"]
  /\ available = [c \in Clients |-> FALSE]
  /\ observed = [c \in Clients |-> NoState]
  /\ version = [c \in Clients |-> 0]
  /\ generation = [c \in Clients |-> 0]
  /\ seen = [c \in Clients |-> TRUE]
  /\ candidate = [c \in Clients |-> NoCandidate]
  /\ configRevision = [c \in Clients |-> 0]
  /\ popup = [c \in Clients |-> TRUE]
  /\ sound = [c \in Clients |-> FALSE]
  /\ muted = [c \in Clients |-> FALSE]
  /\ sinkBusy = [c \in Clients |-> FALSE]
  /\ now = 0
  /\ draw = [c \in Clients |-> 0]
  /\ lastEmitted = [c \in Clients |-> NoDeliveryPosition]
  /\ violations = [c \in Clients |-> {}]

AutomaticEnabled(c) == popup[c] \/ (sound[c] /\ ~muted[c])

DueAt == IF now < MaxTime THEN now + 1 ELSE MaxTime

MakeCandidate(c, from, to, kind) ==
  IF AutomaticEnabled(c)
  THEN [kind |-> kind, generation |-> generation[c], version |-> version[c] + 1,
        from |-> from, to |-> to, due |-> DueAt, draw |-> draw[c] + 1,
        config |-> configRevision[c]]
  ELSE NoCandidate

InstallBaseline(c) ==
  /\ status[c] = "live" /\ ~available[c]
  /\ \E state \in States:
       /\ available' = [available EXCEPT ![c] = TRUE]
       /\ observed' = [observed EXCEPT ![c] = state]
       /\ seen' = [seen EXCEPT ![c] = TRUE]
       /\ candidate' = [candidate EXCEPT ![c] = NoCandidate]
  \* A returning baseline collapses unobserved changes without reusing a
  \* delivery key from this physical generation.
  /\ UNCHANGED <<status, version, generation, configRevision, popup, sound,
                 muted, sinkBusy, now, draw, lastEmitted, violations>>

ObserveWorking(c) ==
  /\ status[c] = "live" /\ available[c] /\ version[c] < MaxVersion
  /\ observed[c] # "working"
  /\ observed' = [observed EXCEPT ![c] = "working"]
  /\ version' = [version EXCEPT ![c] = @ + 1]
  /\ seen' = [seen EXCEPT ![c] = FALSE]
  /\ candidate' = [candidate EXCEPT ![c] = NoCandidate]
  /\ UNCHANGED <<status, available, generation, configRevision, popup, sound,
                 muted, sinkBusy, now, draw, lastEmitted, violations>>

ObserveBlocked(c) ==
  /\ status[c] = "live" /\ available[c] /\ version[c] < MaxVersion
  /\ draw[c] < MaxDraw /\ observed[c] # "blocked"
  /\ LET next == IF observed[c] \in {"idle", "working"}
                  THEN MakeCandidate(c, observed[c], "blocked", "request")
                  ELSE NoCandidate
     IN candidate' = [candidate EXCEPT ![c] = next]
  /\ observed' = [observed EXCEPT ![c] = "blocked"]
  /\ version' = [version EXCEPT ![c] = @ + 1]
  /\ seen' = [seen EXCEPT ![c] = FALSE]
  /\ UNCHANGED <<status, available, generation, configRevision, popup, sound,
                 muted, sinkBusy, now, draw, lastEmitted, violations>>

ObserveIdle(c) ==
  /\ status[c] = "live" /\ available[c] /\ version[c] < MaxVersion
  /\ draw[c] < MaxDraw /\ observed[c] # "idle"
  /\ LET next == IF observed[c] = "working"
                  THEN MakeCandidate(c, "working", "idle", "done")
                  ELSE NoCandidate
     IN candidate' = [candidate EXCEPT ![c] = next]
  /\ observed' = [observed EXCEPT ![c] = "idle"]
  /\ version' = [version EXCEPT ![c] = @ + 1]
  /\ seen' = [seen EXCEPT ![c] = FALSE]
  /\ UNCHANGED <<status, available, generation, configRevision, popup, sound,
                 muted, sinkBusy, now, draw, lastEmitted, violations>>

ObserveUnknown(c) ==
  /\ status[c] = "live" /\ available[c] /\ version[c] < MaxVersion
  /\ observed[c] # "unknown"
  /\ observed' = [observed EXCEPT ![c] = "unknown"]
  /\ version' = [version EXCEPT ![c] = @ + 1]
  /\ seen' = [seen EXCEPT ![c] = FALSE]
  /\ candidate' = [candidate EXCEPT ![c] = NoCandidate]
  /\ UNCHANGED <<status, available, generation, configRevision, popup, sound,
                 muted, sinkBusy, now, draw, lastEmitted, violations>>

RepeatObservation(c) ==
  /\ status[c] = "live" /\ available[c]
  /\ UNCHANGED vars

ReplaceBinding(c) ==
  /\ status[c] = "live" /\ available[c] /\ generation[c] < MaxGeneration
  /\ \E state \in States:
       /\ generation' = [generation EXCEPT ![c] = @ + 1]
       /\ observed' = [observed EXCEPT ![c] = state]
       /\ version' = [version EXCEPT ![c] = 0]
       /\ seen' = [seen EXCEPT ![c] = TRUE]
       /\ candidate' = [candidate EXCEPT ![c] = NoCandidate]
  /\ UNCHANGED <<status, available, configRevision, popup, sound, muted,
                 sinkBusy, now, draw, lastEmitted, violations>>

ObservationUnavailable(c) ==
  /\ status[c] = "live" /\ available[c]
  /\ available' = [available EXCEPT ![c] = FALSE]
  /\ observed' = [observed EXCEPT ![c] = NoState]
  /\ seen' = [seen EXCEPT ![c] = FALSE]
  /\ candidate' = [candidate EXCEPT ![c] = NoCandidate]
  /\ UNCHANGED <<status, version, generation, configRevision, popup, sound,
                 muted, sinkBusy, now, draw, lastEmitted, violations>>

PresentExact(c) ==
  /\ status[c] = "live" /\ available[c] /\ ~seen[c] /\ draw[c] < MaxDraw
  /\ seen' = [seen EXCEPT ![c] = TRUE]
  /\ draw' = [draw EXCEPT ![c] = @ + 1]
  /\ UNCHANGED <<status, available, observed, version, generation, candidate,
                 configRevision, popup, sound, muted, sinkBusy, now,
                 lastEmitted, violations>>

PresentStale(c) ==
  /\ status[c] = "live" /\ available[c] /\ version[c] > 0
  /\ UNCHANGED vars

SuccessfulDraw(c) ==
  /\ status[c] = "live" /\ draw[c] < MaxDraw
  /\ draw' = [draw EXCEPT ![c] = @ + 1]
  /\ UNCHANGED <<status, available, observed, version, generation, seen,
                 candidate, configRevision, popup, sound, muted, sinkBusy,
                 now, lastEmitted, violations>>

AdvanceTime ==
  /\ now < MaxTime
  /\ now' = now + 1
  /\ UNCHANGED <<status, available, observed, version, generation, seen,
                 candidate, configRevision, popup, sound, muted, sinkBusy,
                 draw, lastEmitted, violations>>

ReloadValid(c) ==
  /\ status[c] = "live" /\ configRevision[c] < MaxConfigRevision
  /\ \E nextPopup, nextSound, nextMuted \in BOOLEAN:
       /\ configRevision' = [configRevision EXCEPT ![c] = @ + 1]
       /\ popup' = [popup EXCEPT ![c] = nextPopup]
       /\ sound' = [sound EXCEPT ![c] = nextSound]
       /\ muted' = [muted EXCEPT ![c] = nextMuted]
       /\ candidate' = [candidate EXCEPT ![c] = NoCandidate]
  /\ UNCHANGED <<status, available, observed, version, generation, seen,
                 sinkBusy, now, draw, lastEmitted, violations>>

ReloadInvalid(c) ==
  /\ status[c] = "live"
  /\ UNCHANGED vars

SinkBusy(c) ==
  /\ status[c] = "live" /\ ~sinkBusy[c]
  /\ sinkBusy' = [sinkBusy EXCEPT ![c] = TRUE]
  /\ UNCHANGED <<status, available, observed, version, generation, seen,
                 candidate, configRevision, popup, sound, muted, now, draw,
                 lastEmitted, violations>>

SinkAvailable(c) ==
  /\ status[c] = "live" /\ sinkBusy[c]
  /\ sinkBusy' = [sinkBusy EXCEPT ![c] = FALSE]
  /\ UNCHANGED <<status, available, observed, version, generation, seen,
                 candidate, configRevision, popup, sound, muted, now, draw,
                 lastEmitted, violations>>

Ready(c) ==
  /\ status[c] = "live" /\ candidate[c] # NoCandidate
  /\ candidate[c].due <= now /\ candidate[c].draw <= draw[c]
  /\ candidate[c].generation = generation[c]
  /\ candidate[c].version = version[c]
  /\ candidate[c].to = observed[c]
  /\ candidate[c].config = configRevision[c]
  /\ ~seen[c] /\ ~sinkBusy[c] /\ AutomaticEnabled(c)

Emit(c) ==
  /\ Ready(c)
  /\ LET effect ==
       [client |-> c, generation |-> generation[c], version |-> version[c],
        kind |-> candidate[c].kind, from |-> candidate[c].from,
        to |-> candidate[c].to, config |-> configRevision[c],
        popup |-> popup[c], sound |-> sound[c] /\ ~muted[c],
        muted |-> muted[c], seenAtDelivery |-> seen[c],
        sinkBusyAtDelivery |-> sinkBusy[c]]
       previous == lastEmitted[c]
     IN /\ lastEmitted' =
              [lastEmitted EXCEPT ![c] = DeliveryPosition(effect)]
        /\ violations' =
              [violations EXCEPT
                 ![c] = @ \cup DeliveryViolations(effect, previous)]
  /\ candidate' = [candidate EXCEPT ![c] = NoCandidate]
  /\ UNCHANGED <<status, available, observed, version, generation, seen,
                 configRevision, popup, sound, muted, sinkBusy, now, draw>>

SuppressSeen(c) ==
  /\ status[c] = "live" /\ candidate[c] # NoCandidate /\ seen[c]
  /\ candidate' = [candidate EXCEPT ![c] = NoCandidate]
  /\ UNCHANGED <<status, available, observed, version, generation, seen,
                 configRevision, popup, sound, muted, sinkBusy, now, draw,
                 lastEmitted, violations>>

Retire(c) ==
  /\ status[c] = "live"
  /\ status' = [status EXCEPT ![c] = "retired"]
  /\ available' = [available EXCEPT ![c] = FALSE]
  /\ candidate' = [candidate EXCEPT ![c] = NoCandidate]
  /\ UNCHANGED <<observed, version, generation, seen, configRevision, popup,
                 sound, muted, sinkBusy, now, draw, lastEmitted, violations>>

RetiredStutter ==
  /\ \A c \in Clients : status[c] = "retired"
  /\ UNCHANGED vars

Next ==
  \/ \E c \in Clients : InstallBaseline(c)
  \/ \E c \in Clients : ObserveWorking(c)
  \/ \E c \in Clients : ObserveBlocked(c)
  \/ \E c \in Clients : ObserveIdle(c)
  \/ \E c \in Clients : ObserveUnknown(c)
  \/ \E c \in Clients : RepeatObservation(c)
  \/ \E c \in Clients : ReplaceBinding(c)
  \/ \E c \in Clients : ObservationUnavailable(c)
  \/ \E c \in Clients : PresentExact(c)
  \/ \E c \in Clients : PresentStale(c)
  \/ \E c \in Clients : SuccessfulDraw(c)
  \/ AdvanceTime
  \/ \E c \in Clients : ReloadValid(c)
  \/ \E c \in Clients : ReloadInvalid(c)
  \/ \E c \in Clients : SinkBusy(c)
  \/ \E c \in Clients : SinkAvailable(c)
  \/ \E c \in Clients : Emit(c)
  \/ \E c \in Clients : SuppressSeen(c)
  \/ \E c \in Clients : Retire(c)
  \/ RetiredStutter

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(AdvanceTime)
  /\ \A c \in Clients : WF_vars(SuccessfulDraw(c))
  /\ \A c \in Clients : WF_vars(SinkAvailable(c))
  /\ \A c \in Clients : SF_vars(Emit(c))
  /\ \A c \in Clients : WF_vars(SuppressSeen(c))

TypeOK ==
  /\ status \in [Clients -> {"live", "retired"}]
  /\ available \in [Clients -> BOOLEAN]
  /\ observed \in [Clients -> States \cup {NoState}]
  /\ version \in [Clients -> 0..MaxVersion]
  /\ generation \in [Clients -> 0..MaxGeneration]
  /\ seen \in [Clients -> BOOLEAN]
  /\ candidate \in [Clients -> CandidateRecords \cup {NoCandidate}]
  /\ configRevision \in [Clients -> 0..MaxConfigRevision]
  /\ popup \in [Clients -> BOOLEAN] /\ sound \in [Clients -> BOOLEAN]
  /\ muted \in [Clients -> BOOLEAN] /\ sinkBusy \in [Clients -> BOOLEAN]
  /\ now \in 0..MaxTime /\ draw \in [Clients -> 0..MaxDraw]
  /\ lastEmitted \in
       [Clients -> DeliveryPositions \cup {NoDeliveryPosition}]
  /\ violations \in [Clients -> SUBSET ViolationKinds]

NoBaselineOrReplacementCandidate ==
  \A c \in Clients : (version[c] = 0 \/ ~available[c]) => candidate[c] = NoCandidate

CandidateIsExactAllowedTrigger ==
  \A c \in Clients : candidate[c] # NoCandidate =>
    /\ candidate[c].generation = generation[c]
    /\ candidate[c].version = version[c]
    /\ candidate[c].to = observed[c]
    /\ candidate[c].config = configRevision[c]
    /\ \/ candidate[c].kind = "done" /\ candidate[c].from = "working"
          /\ candidate[c].to = "idle"
       \/ candidate[c].kind = "request" /\ candidate[c].from \in {"idle", "working"}
          /\ candidate[c].to = "blocked"

DeliveredHasAllowedTrigger ==
  \A c \in Clients : "bad-trigger" \notin violations[c]

NoSeenOrBusyDelivery ==
  \A c \in Clients : "seen-or-busy" \notin violations[c]

MutedAgentHasNoAutomaticSound ==
  \A c \in Clients : "muted-sound" \notin violations[c]

DeliveryRespectsPublishedPolicy ==
  \A c \in Clients : "disabled-policy" \notin violations[c]

AtMostOncePerClientVersionKind ==
  \A c \in Clients : "position-reuse" \notin violations[c]

CurrentPositionDoesNotPrecedeLastDelivery ==
  \A c \in Clients :
    LET previous == lastEmitted[c]
    IN \/ previous = NoDeliveryPosition
       \/ generation[c] > previous.generation
       \/ generation[c] = previous.generation /\ version[c] >= previous.version

PendingIsBounded == Cardinality({c \in Clients : candidate[c] # NoCandidate}) <= Cardinality(Clients)

RetiredClientHasNoEffects ==
  \A c \in Clients : status[c] = "retired" => candidate[c] = NoCandidate

\* Under the fairness assumptions in Spec, logical time, a successful draw and a
\* temporarily busy sink cannot strand a retained candidate forever. Optional
\* observation/configuration/rebind/retirement inputs may instead cancel it.
ReadyCandidateSettles == \A c \in Clients : Ready(c) ~> candidate[c] = NoCandidate

=============================================================================
