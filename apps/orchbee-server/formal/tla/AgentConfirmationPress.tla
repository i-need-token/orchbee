---------------------- MODULE AgentConfirmationPress ----------------------
EXTENDS Naturals, Sequences, FiniteSets

\* One terminal running one Agent. Four parties share no clock: the Agent
\* raises, draws and resolves confirmation prompts and reads keys; a person
\* types and moves the pointer; the Server samples the screen and decides; the
\* Host publishes input. The Server decides on a captured frame and sends a key
\* message naming that frame and the revision at which it first saw the
\* prompt. The Host writes the accept key only if its screen is still that
\* frame, no person acted since the prompt was first seen, and no pointer move
\* reached the Agent within the quiet time. Messages are delivered in any
\* order, late, or never (a message that stays under way), and the Server may
\* stop waiting for one that is still under way.
\*
\* A person's key or a pointer move without a button may move the prompt's
\* highlight off the one-time accept, and a later one may move it back; the
\* prompt is then not routine until it is back. Such a pointer move is not a
\* person's input: the Host records it apart, and it only delays the key.
\*
\* Layout matching and the path check are one pure function from a screen to
\* "routine", "hold" or "work" and have rule tests; a command is not judged.
\* Durations are reduced to what they guard: the one-second settle to a sample
\* count, and the Host's quiet times after a person's input and after a
\* pointer move to flags that HostQuiet and PointerQuiet may clear. The longer
\* settle for a prompt whose identity was just pressed only delays Decide and
\* is not modeled. A prompt that leaves the screen and returns with the same
\* identity may keep the revision at which it was first seen, or may not,
\* since the Server keeps it only for a while.
\*
\* Three assumptions about Agents are written into the actions and stated in
\* the owning reference:
\*   - Draw: a drawn prompt leaves the screen only after the Agent resolved it
\*     or moved its highlight.
\*   - HostQuiet: within the quiet time an Agent reads a pending person key
\*     and draws what that key changed.
\*   - PointerQuiet: within the quiet time an Agent reads a pending pointer
\*     move and draws what that move changed.
CONSTANTS
  TwoIdentities,  \* whether two different routine prompts exist
  MaxInst,        \* prompts the Agent raises
  MaxRev,         \* content revisions of the Host's screen
  MaxPerson,      \* times a person types or clicks
  MaxPointer,     \* times a pointer move without a button reaches the Agent
  MaxMsg,         \* key and hold messages the Server sends
  PressLimit,     \* presses per prompt identity; the implementation uses three
  Orderly         \* no redraw, Agent timeout, late message or turning off: the
                  \* variant that counts presses up to the implementation's limit

Routine == IF TwoIdentities THEN {"ra", "rb"} ELSE {"ra"}
Contents == Routine \cup {"hold"}

VARIABLES agent, host, server, net, person, ghost
vars == <<agent, host, server, net, person, ghost>>

NoCandidate == [ident |-> "none", samples |-> 0, first |-> 0, personsFirst |-> 0, sent |-> 0]

Init ==
  /\ agent = [prompt |-> 0, raised |-> 0, content |-> [i \in 1..MaxInst |-> "none"],
              always |-> [i \in 1..MaxInst |-> FALSE]]
  /\ host = [content |-> "work", inst |-> 0, rev |-> 1, personRev |-> 0, typedRev |-> 0,
             recent |-> FALSE, persons |-> 0, pointers |-> 0, pointerRecent |-> FALSE,
             pty |-> <<>>]
  /\ server = [choice |-> "off", holdId |-> 0, holdDone |-> TRUE,
               frame |-> [content |-> "work", rev |-> 0],
               cand |-> NoCandidate, left |-> NoCandidate,
               presses |-> [c \in Routine |-> 0], waiting |-> 0]
  /\ net = {}
  /\ person = 0
  /\ ghost = [writes |-> {}, misread |-> {}, spont |-> {}, stolen |-> {}, always |-> {},
              next |-> 1]

\* ---- Agent ----
\* A prompt whose highlight is off the one-time accept shows as "always".
View == IF agent.prompt = 0 THEN "work"
        ELSE IF agent.always[agent.prompt] THEN "always"
        ELSE agent.content[agent.prompt]

\* The routine identities are interchangeable, so the first prompt is never "rb".
Raise(c) ==
  /\ agent.prompt = 0 /\ agent.raised < MaxInst
  /\ agent.raised = 0 => c # "rb"
  /\ agent' = [agent EXCEPT !.raised = @ + 1, !.prompt = agent.raised + 1,
                 !.content[agent.raised + 1] = c]
  /\ UNCHANGED <<host, server, net, person, ghost>>

Draw ==
  /\ host.rev < MaxRev
  /\ host.content # View \/ host.inst # agent.prompt
  /\ host' = [host EXCEPT !.content = View, !.inst = agent.prompt, !.rev = @ + 1]
  /\ UNCHANGED <<agent, server, net, person, ghost>>

\* An animation or other output that leaves the prompt as it is.
Redraw ==
  /\ ~Orderly /\ host.rev < MaxRev
  /\ host.content = View /\ host.inst = agent.prompt
  /\ host' = [host EXCEPT !.rev = @ + 1]
  /\ UNCHANGED <<agent, server, net, person, ghost>>

\* The accept key resolves the prompt it was written for, confirming whichever
\* option is highlighted. A person's key, or an accept key that meets another
\* prompt, resolves the current prompt or is ignored by it. A person's key or a
\* pointer move may move the highlight; a pointer move never resolves.
ReadKey(effect) ==
  /\ host.pty # <<>>
  /\ LET k == Head(host.pty)
         auto == k.src = "auto"
         aimed == auto /\ k.for = agent.prompt
         astray == auto /\ k.for # agent.prompt
         shown == agent.prompt # 0
         ends == shown /\ effect = "resolve"
     IN /\ aimed => effect = "resolve"
        /\ ~shown => effect = "resolve"
        /\ effect \in {"away", "back"} => shown /\ ~auto
        /\ k.src = "pointer" /\ shown => effect # "resolve"
        /\ host' = [host EXCEPT !.pty = Tail(@)]
        /\ agent' = CASE ends -> [agent EXCEPT !.prompt = 0]
                      [] effect = "away" -> [agent EXCEPT !.always[agent.prompt] = TRUE]
                      [] effect = "back" -> [agent EXCEPT !.always[agent.prompt] = FALSE]
                      [] OTHER -> agent
        /\ ghost' = [ghost EXCEPT
             !.misread = IF astray THEN @ \cup {[id |-> k.id, for |-> k.for]} ELSE @,
             !.stolen = IF astray /\ ends THEN @ \cup {agent.prompt} ELSE @,
             !.always = IF auto /\ ends /\ agent.always[agent.prompt]
                        THEN @ \cup {agent.prompt} ELSE @]
  /\ UNCHANGED <<server, net, person>>

\* The Agent's own timeout, or an answer from a companion surface.
Spontaneous ==
  /\ ~Orderly /\ agent.prompt # 0
  /\ agent' = [agent EXCEPT !.prompt = 0]
  /\ ghost' = [ghost EXCEPT !.spont = @ \cup {agent.prompt}]
  /\ UNCHANGED <<host, server, net, person>>

\* ---- Person ----
\* A key reaches the Agent. A click that encodes to nothing reaches nobody,
\* and the Host records both alike.
PersonType(bytes) ==
  /\ person < MaxPerson
  /\ person' = person + 1
  /\ host' = [host EXCEPT
       !.pty = IF bytes THEN Append(@, [src |-> "person", for |-> 0, id |-> 0]) ELSE @,
       !.personRev = host.rev, !.recent = TRUE, !.persons = @ + 1]
  /\ UNCHANGED <<agent, server, net, ghost>>

HostQuiet ==
  /\ host.recent
  /\ \A i \in 1..Len(host.pty): host.pty[i].src # "person"
  /\ host.content = View /\ host.inst = agent.prompt
  /\ host' = [host EXCEPT !.recent = FALSE]
  /\ UNCHANGED <<agent, server, net, person, ghost>>

\* A pointer move without a button reaches an Agent that tracks the pointer.
\* The Host records when, not who: it is no person's input.
PointerMove ==
  /\ host.pointers < MaxPointer
  /\ host' = [host EXCEPT
       !.pty = Append(@, [src |-> "pointer", for |-> 0, id |-> 0]),
       !.pointers = @ + 1, !.pointerRecent = TRUE]
  /\ UNCHANGED <<agent, server, net, person, ghost>>

PointerQuiet ==
  /\ host.pointerRecent
  /\ \A i \in 1..Len(host.pty): host.pty[i].src # "pointer"
  /\ host.content = View /\ host.inst = agent.prompt
  /\ host' = [host EXCEPT !.pointerRecent = FALSE]
  /\ UNCHANGED <<agent, server, net, person, ghost>>

\* ---- Server ----
TurnOn ==
  /\ server.choice = "off"
  /\ server' = [server EXCEPT !.choice = "on"]
  /\ UNCHANGED <<agent, host, net, person, ghost>>

\* Every way of turning off sends a hold, which makes the Host refuse every
\* key decided earlier. Off is confirmed when the Host has applied that hold.
SwitchOff ==
  /\ ghost.next <= MaxMsg
  /\ server' = [server EXCEPT !.choice = "off", !.holdId = ghost.next, !.holdDone = FALSE]
  /\ net' = net \cup {[id |-> ghost.next, kind |-> "hold"]}
  /\ ghost' = [ghost EXCEPT !.next = @ + 1]

TurnOff ==
  /\ ~Orderly /\ server.choice = "on"
  /\ SwitchOff
  /\ UNCHANGED <<agent, host, person>>

\* A sample: the frame and the prompt it continues or starts. A pressed
\* prompt stays the same prompt until a sample shows the terminal without it.
\* A prompt that left without a press is remembered, and when it returns with
\* the same identity it may keep its first revision (keep).
Capture(keep) ==
  /\ LET c == host.content
         back == keep /\ server.left.ident = c /\ server.left.sent = 0
     IN server' = [server EXCEPT
          !.frame = [content |-> c, rev |-> host.rev],
          !.cand = IF c \in Routine
                   THEN IF server.cand.ident = c
                        THEN [server.cand EXCEPT !.samples = IF @ < 2 THEN @ + 1 ELSE @]
                        ELSE IF back
                        THEN [server.left EXCEPT !.samples = 1]
                        ELSE [ident |-> c, samples |-> 1, first |-> host.rev,
                              personsFirst |-> host.persons, sent |-> 0]
                   ELSE NoCandidate,
          !.left = IF c \in Routine THEN NoCandidate
                   ELSE IF server.cand.ident \in Routine THEN server.cand
                   ELSE server.left]
  /\ UNCHANGED <<agent, host, net, person, ghost>>

Settled ==
  /\ server.choice = "on" /\ server.waiting = 0
  /\ server.cand.ident \in Routine /\ server.cand.samples = 2
  /\ server.cand.sent = 0

\* The press is counted before its outcome is known.
Decide ==
  /\ Settled
  /\ server.presses[server.cand.ident] < PressLimit
  /\ ghost.next <= MaxMsg
  /\ server' = [server EXCEPT !.cand.sent = ghost.next,
                 !.presses[server.cand.ident] = @ + 1,
                 !.waiting = ghost.next]
  /\ net' = net \cup {[id |-> ghost.next, kind |-> "key", rev |-> server.frame.rev,
                       first |-> server.cand.first, ident |-> server.cand.ident,
                       personsFirst |-> server.cand.personsFirst]}
  /\ ghost' = [ghost EXCEPT !.next = @ + 1]
  /\ UNCHANGED <<agent, host, person>>

Stop ==
  /\ Settled
  /\ server.presses[server.cand.ident] = PressLimit
  /\ SwitchOff
  /\ UNCHANGED <<agent, host, person>>

\* The Server stops waiting; the message may still arrive.
Timeout ==
  /\ ~Orderly /\ server.waiting # 0
  /\ server' = [server EXCEPT !.waiting = 0]
  /\ UNCHANGED <<agent, host, net, person, ghost>>

\* ---- Host gate ----
GateHolds(m) ==
  /\ host.rev = m.rev
  /\ host.typedRev # m.rev
  /\ host.personRev < m.first
  /\ ~host.recent
  /\ ~host.pointerRecent

WriteKey(m) ==
  /\ m.kind = "key" /\ GateHolds(m)
  /\ host' = [host EXCEPT !.typedRev = m.rev,
       !.pty = Append(@, [src |-> "auto", for |-> host.inst, id |-> m.id])]
  /\ ghost' = [ghost EXCEPT
       !.writes = @ \cup {[id |-> m.id, inst |-> host.inst, content |-> host.content,
                           matched |-> m.ident,
                           personSince |-> host.persons # m.personsFirst,
                           behindPerson |-> \E i \in 1..Len(host.pty): host.pty[i].src = "person"]}]
  /\ net' = net \ {m}
  /\ server' = IF server.waiting = m.id THEN [server EXCEPT !.waiting = 0] ELSE server
  /\ UNCHANGED <<agent, person>>

\* A refusal the Server still waits for returns the press: nothing was written.
RefuseKey(m) ==
  /\ m.kind = "key" /\ ~GateHolds(m)
  /\ net' = net \ {m}
  /\ server' = IF server.waiting = m.id
       THEN [server EXCEPT !.waiting = 0,
              !.presses[m.ident] = @ - 1,
              !.cand.sent = IF @ = m.id THEN 0 ELSE @]
       ELSE server
  /\ UNCHANGED <<agent, host, person, ghost>>

\* A hold counts as a person's action on the current screen.
ApplyHold(m) ==
  /\ m.kind = "hold"
  /\ net' = net \ {m}
  /\ host' = [host EXCEPT !.personRev = host.rev, !.recent = TRUE]
  /\ server' = IF server.choice = "off" /\ server.holdId = m.id
       THEN [server EXCEPT !.holdDone = TRUE] ELSE server
  /\ UNCHANGED <<agent, person, ghost>>

GateWrite == \E m \in net: WriteKey(m)
GateRefuse == \E m \in net: RefuseKey(m)
HoldApply == \E m \in net: ApplyHold(m)

Next ==
  \/ \E c \in Contents: Raise(c)
  \/ Draw \/ Redraw \/ Spontaneous
  \/ \E effect \in {"resolve", "ignore", "away", "back"}: ReadKey(effect)
  \/ \E bytes \in BOOLEAN: PersonType(bytes)
  \/ HostQuiet \/ PointerMove \/ PointerQuiet \/ TurnOn \/ TurnOff
  \/ \E keep \in BOOLEAN: Capture(keep)
  \/ Decide \/ Stop \/ Timeout
  \/ GateWrite \/ GateRefuse \/ HoldApply
Spec == Init /\ [][Next]_vars

TypeOK ==
  /\ agent.prompt \in 0..MaxInst /\ agent.raised \in 0..MaxInst
  /\ host.rev \in 1..MaxRev /\ host.personRev \in 0..MaxRev /\ host.typedRev \in 0..MaxRev
  /\ host.content \in Contents \cup {"work", "always"} /\ host.inst \in 0..MaxInst
  /\ host.recent \in BOOLEAN /\ host.persons \in 0..MaxPerson
  /\ host.pointerRecent \in BOOLEAN /\ host.pointers \in 0..MaxPointer
  /\ server.choice \in {"off", "on"} /\ server.holdDone \in BOOLEAN
  /\ server.cand.ident \in Routine \cup {"none"} /\ server.cand.samples \in 0..2
  /\ server.left.ident \in Routine \cup {"none"}
  /\ \A c \in Routine: server.presses[c] \in 0..PressLimit
  /\ person \in 0..MaxPerson
  /\ \A m \in net: m.id \in 1..MaxMsg

\* Every written key was written while the Host's screen was the routine
\* prompt the Server matched.
MatchedScreen ==
  \A w \in ghost.writes: w.content = w.matched /\ w.content \in Routine /\ w.inst # 0
OncePerPrompt ==
  \A w1, w2 \in ghost.writes: w1.inst = w2.inst => w1.id = w2.id
\* No key is written once a person typed or clicked after the prompt was
\* first seen, nor behind a person's key the Agent has not read.
PersonFirst == \A w \in ghost.writes: ~w.personSince /\ ~w.behindPerson
AcceptKeyOnly ==
  \A i \in 1..Len(host.pty): host.pty[i].src = "auto" =>
    \E w \in ghost.writes: w.id = host.pty[i].id
\* No key Orchbee wrote confirms a highlight that a person's key or a pointer
\* move took off the one-time accept.
AcceptedOnce == ghost.always = {}
\* A key reaches another prompt only when its own prompt was resolved without
\* a key, or by a key that had itself gone astray for that reason.
LandsWhereAimed == \A k \in ghost.misread: k.for \in ghost.spont \cup ghost.stolen
AstrayNeedsSpontaneous == ghost.misread # {} => ghost.spont # {}
RepeatsStop ==
  \A c \in Routine: Cardinality({w \in ghost.writes: w.matched = c}) <= PressLimit

\* Once the Host confirmed Off, nothing is written until it is turned on
\* again, whatever was decided before and however late it arrives.
OffMeansOff ==
  [][server.choice = "off" /\ server.holdDone => ghost'.writes = ghost.writes]_vars
\* A key written while Off is not yet confirmed was decided before Off.
OffDecidesNothing ==
  [][server.choice = "off" => \A m \in net' \ net: m.kind = "hold"]_vars
=============================================================================
