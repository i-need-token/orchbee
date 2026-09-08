---------------------- MODULE LocalClientAdmission ----------------------
EXTENDS Naturals, TLC

\* One caller-owned absolute deadline races all phases of one OBLC admission.
\* External readiness, peer proof, partial I/O and responses are not fair.
\* Counters expose one connection attempt and one hello; chunks are a finite
\* abstraction of partial progress, not OBLC byte counts or a time budget.

Phases == {"ready", "connecting", "peer", "writing", "reading",
           "decoded", "restoring", "accepted", "failed"}
Pending == Phases \ {"accepted", "failed"}
Responses == {"none", "accepted", "rejected", "malformed", "unexpected"}

VARIABLES phase, expired, socketOwned, connections, peerVerified,
          helloChunks, responseChunks, response, outcome
vars == <<phase, expired, socketOwned, connections, peerVerified,
          helloChunks, responseChunks, response, outcome>>

Init ==
  /\ phase = "ready"
  /\ expired = FALSE
  /\ socketOwned = FALSE
  /\ connections = 0
  /\ peerVerified = FALSE
  /\ helloChunks = 0
  /\ responseChunks = 0
  /\ response = "none"
  /\ outcome = "none"

RejectInvalidCohort ==
  /\ phase = "ready"
  /\ phase' = "failed"
  /\ outcome' = "invalid-cohort"
  /\ UNCHANGED <<expired, socketOwned, connections, peerVerified,
       helloChunks, responseChunks, response>>

BeginConnect ==
  /\ phase = "ready" /\ ~expired
  /\ phase' = "connecting"
  /\ socketOwned' = TRUE
  /\ connections' = 1
  /\ UNCHANGED <<expired, peerVerified, helloChunks, responseChunks, response, outcome>>

ConnectionReady ==
  /\ phase = "connecting" /\ ~expired
  /\ phase' = "peer"
  /\ UNCHANGED <<expired, socketOwned, connections, peerVerified,
       helloChunks, responseChunks, response, outcome>>

VerifyPeer ==
  /\ phase = "peer" /\ ~expired
  /\ phase' = "writing"
  /\ peerVerified' = TRUE
  /\ UNCHANGED <<expired, socketOwned, connections,
       helloChunks, responseChunks, response, outcome>>

RejectPeer ==
  /\ phase = "peer" /\ ~expired
  /\ phase' = "failed"
  /\ socketOwned' = FALSE
  /\ outcome' = "wrong-peer"
  /\ UNCHANGED <<expired, connections, peerVerified,
       helloChunks, responseChunks, response>>

WriteHelloChunk ==
  /\ phase = "writing" /\ ~expired
  /\ helloChunks < 2
  /\ helloChunks' = helloChunks + 1
  /\ phase' = IF helloChunks' = 2 THEN "reading" ELSE phase
  /\ UNCHANGED <<expired, socketOwned, connections, peerVerified,
       responseChunks, response, outcome>>

ReadResponseChunk ==
  /\ phase = "reading" /\ ~expired
  /\ responseChunks < 2
  /\ responseChunks' = responseChunks + 1
  /\ UNCHANGED <<phase, expired, socketOwned, connections, peerVerified,
       helloChunks, response, outcome>>

DecodeResponse ==
  /\ phase = "reading" /\ ~expired /\ responseChunks = 2
  /\ response' \in Responses \ {"none"}
  /\ phase' = "decoded"
  /\ UNCHANGED <<expired, socketOwned, connections, peerVerified,
       helloChunks, responseChunks, outcome>>

AcceptResponse ==
  /\ phase = "decoded" /\ ~expired /\ response = "accepted"
  /\ phase' = "restoring"
  /\ UNCHANGED <<expired, socketOwned, connections, peerVerified,
       helloChunks, responseChunks, response, outcome>>

RejectResponse ==
  /\ phase = "decoded" /\ ~expired /\ response \in {"rejected", "malformed", "unexpected"}
  /\ phase' = "failed"
  /\ socketOwned' = FALSE
  /\ outcome' = response
  /\ UNCHANGED <<expired, connections, peerVerified,
       helloChunks, responseChunks, response>>

ReturnConnection ==
  /\ phase = "restoring" /\ ~expired
  /\ phase' = "accepted"
  /\ outcome' = "accepted"
  /\ UNCHANGED <<expired, socketOwned, connections, peerVerified,
       helloChunks, responseChunks, response>>

IoFails ==
  /\ phase \in Pending \ {"ready"} /\ ~expired
  /\ phase' = "failed"
  /\ socketOwned' = FALSE
  /\ outcome' = "io"
  /\ UNCHANGED <<expired, connections, peerVerified,
       helloChunks, responseChunks, response>>

DeadlineExpires ==
  /\ phase \in Pending /\ ~expired
  /\ expired' = TRUE
  /\ UNCHANGED <<phase, socketOwned, connections, peerVerified,
       helloChunks, responseChunks, response, outcome>>

ObserveDeadline ==
  /\ phase \in Pending /\ expired
  /\ phase' = "failed"
  /\ socketOwned' = FALSE
  /\ outcome' = "timeout"
  /\ UNCHANGED <<expired, connections, peerVerified,
       helloChunks, responseChunks, response>>

Next ==
  \/ RejectInvalidCohort \/ BeginConnect \/ ConnectionReady
  \/ VerifyPeer \/ RejectPeer \/ WriteHelloChunk \/ ReadResponseChunk
  \/ DecodeResponse \/ AcceptResponse \/ RejectResponse \/ ReturnConnection
  \/ IoFails \/ DeadlineExpires \/ ObserveDeadline

Spec == Init /\ [][Next]_vars
  /\ WF_vars(DeadlineExpires) /\ WF_vars(ObserveDeadline)

TypeOK ==
  /\ phase \in Phases /\ expired \in BOOLEAN /\ socketOwned \in BOOLEAN
  /\ connections \in 0..1 /\ peerVerified \in BOOLEAN
  /\ helloChunks \in 0..2 /\ responseChunks \in 0..2
  /\ response \in Responses
  /\ outcome \in {"none", "invalid-cohort", "wrong-peer", "accepted",
                  "rejected", "malformed", "unexpected", "io", "timeout"}

HelloRequiresVerifiedPeer == helloChunks > 0 => peerVerified /\ connections = 1
ResponseFollowsOneHello == responseChunks > 0 => helloChunks = 2 /\ connections = 1
AcceptedIsExactAndTimely ==
  phase = "accepted" =>
    /\ ~expired /\ socketOwned /\ peerVerified /\ connections = 1
    /\ helloChunks = 2 /\ responseChunks = 2 /\ response = "accepted"
    /\ outcome = "accepted"
FailureReleasesCandidate == phase = "failed" => ~socketOwned
InvalidCohortHasNoSocket == outcome = "invalid-cohort" => connections = 0
NoReconnect == [][connections' >= connections]_vars
DeadlineNeverRenews == [][expired => expired']_vars
SettledNeverReenters ==
  [][(phase \in {"accepted", "failed"} => (vars' = vars))]_vars
AdmissionEventuallySettles == <>(phase \in {"accepted", "failed"})

=============================================================================
