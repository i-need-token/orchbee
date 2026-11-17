----------------------- MODULE ProjectionRecordResync -----------------------
EXTENDS Naturals, Sequences

\* One pane's viewer. The Server routes frame records one flight at a time and
\* other records (lifecycle, history, blocked) beside them; the Client answers
\* each frame with FrameApplied or NeedSnapshot naming that flight. A record the
\* Client rejects is dropped and the pane's frame is invalidated. When the
\* rejected record still names its frame flight the Client asks for a snapshot;
\* a rejected other record leaves the next frame to bring the snapshot.
\* Consecutive resyncs without an applied frame are bounded; past the bound, or
\* when the Client cannot tell whether a record was a frame, only this pane
\* stops. The Session has no retirement action on this path, and other panes
\* keep their own counters, so neither is modeled.
\*
\* The claim implementation traces do not expose: the Server never waits
\* forever on a flight the running pane cannot answer, and the Client never
\* sends feedback the Server would refuse (a refusal releases the viewer).
\* Mapping: SendFrame/SendOther = Server viewer projection routing;
\* CorruptPayload/CorruptEnvelope = a record the Client's decoders refuse;
\* Receive = UI poll-result intake and pane resync; Deliver = the executor's
\* causal feedback and the Server's feedback admission.
CONSTANTS MaxSeq, MaxFaults, Bound

VARIABLES srv, chan, cli, out, faults
vars == <<srv, chan, cli, out, faults>>

None == [kind |-> "none", seq |-> 0]
Frames == {"full", "delta"}

Init ==
  /\ srv = [flight |-> 0, last |-> 0, snapshot |-> TRUE]
  /\ chan = <<>>
  /\ cli = [mode |-> "live", frame |-> FALSE, resyncs |-> 0]
  /\ out = None
  /\ faults = 0

SendFrame ==
  /\ srv.flight = 0 /\ srv.last < MaxSeq /\ Len(chan) < 2
  /\ LET seq == srv.last + 1 IN
     /\ srv' = [srv EXCEPT !.flight = seq, !.last = seq]
     /\ chan' = Append(chan,
          [kind |-> IF srv.snapshot THEN "full" ELSE "delta", seq |-> seq, bad |-> "none"])
  /\ UNCHANGED <<cli, out, faults>>

SendOther ==
  /\ Len(chan) < 2
  /\ chan' = Append(chan, [kind |-> "other", seq |-> 0, bad |-> "none"])
  /\ UNCHANGED <<srv, cli, out, faults>>

\* The route stays readable: the Server inspected it before routing.
CorruptPayload ==
  /\ faults < MaxFaults
  /\ \E i \in 1..Len(chan) :
       /\ chan[i].bad = "none"
       /\ chan' = [chan EXCEPT ![i].bad = "payload"]
  /\ faults' = faults + 1
  /\ UNCHANGED <<srv, cli, out>>
\* Nothing in the record is readable.
CorruptEnvelope ==
  /\ faults < MaxFaults
  /\ \E i \in 1..Len(chan) :
       /\ chan[i].bad = "none"
       /\ chan' = [chan EXCEPT ![i].bad = "envelope"]
  /\ faults' = faults + 1
  /\ UNCHANGED <<srv, cli, out>>

Receive ==
  /\ chan # <<>> /\ out = None /\ cli.mode # "stopped"
  /\ LET r == Head(chan)
         \* Only a readable envelope tells the Client a record was a frame.
         frame == r.kind \in Frames /\ r.bad # "envelope"
         count == cli.resyncs + 1
     IN /\ chan' = Tail(chan)
        /\ CASE r.bad = "none" /\ frame /\ (r.kind = "full" \/ cli.frame) ->
                  /\ cli' = [mode |-> "live", frame |-> TRUE, resyncs |-> 0]
                  /\ out' = [kind |-> "applied", seq |-> r.seq]
             [] r.bad = "none" /\ frame ->
                  \* An unappliable delta: the ordinary snapshot request.
                  /\ cli' = [cli EXCEPT !.frame = FALSE]
                  /\ out' = [kind |-> "snapshot", seq |-> r.seq]
             [] r.bad = "none" ->
                  UNCHANGED <<cli, out>>
             [] r.bad = "envelope" \/ count > Bound ->
                  /\ cli' = [cli EXCEPT !.mode = "stopped", !.frame = FALSE]
                  /\ UNCHANGED out
             [] frame ->
                  /\ cli' = [mode |-> "resyncing", frame |-> FALSE, resyncs |-> count]
                  /\ out' = [kind |-> "snapshot", seq |-> r.seq]
             [] OTHER ->
                  /\ cli' = [mode |-> "resyncing", frame |-> FALSE, resyncs |-> count]
                  /\ UNCHANGED out
  /\ UNCHANGED <<srv, faults>>

Deliver ==
  /\ out # None /\ srv.flight = out.seq
  /\ srv' = [srv EXCEPT !.flight = 0, !.snapshot = (out.kind = "snapshot")]
  /\ out' = None
  /\ UNCHANGED <<chan, cli, faults>>

Next == SendFrame \/ SendOther \/ CorruptPayload \/ CorruptEnvelope \/ Receive \/ Deliver
Spec == Init /\ [][Next]_vars /\ WF_vars(Receive) /\ WF_vars(Deliver)

TypeOK ==
  /\ cli.mode \in {"live", "resyncing", "stopped"}
  /\ cli.frame \in BOOLEAN
  /\ srv.flight \in 0..MaxSeq /\ srv.last \in 0..MaxSeq
  /\ faults \in 0..MaxFaults
  /\ Len(chan) <= 2
BoundedResyncs == cli.resyncs <= Bound
\* Every answer the Client sends names the Server's current flight, so a
\* rejected record never escalates to a refused answer and a released viewer.
FeedbackNamesFlight == out # None => out.seq = srv.flight
StoppedSendsNothing == cli.mode = "stopped" => out = None
FrameOnlyWhenLive == cli.frame => cli.mode = "live"
\* A running pane always answers the frame the Server is waiting on.
FlightsAnswered ==
  (srv.flight # 0 /\ cli.mode # "stopped") ~> (srv.flight = 0 \/ cli.mode = "stopped")
=============================================================================
