-------------------- MODULE WindowsCheckpointRestoreNotice --------------------
EXTENDS Naturals, TLC
CONSTANT MaxActivation
Outcomes == {"None", "Recovered", "Empty"}
VARIABLES activation, stage, loaded, diskActivation, diskNotice, ready,
          clientActivation, clientNotice, connected, lastEvent
vars == <<activation, stage, loaded, diskActivation, diskNotice, ready,
          clientActivation, clientNotice, connected, lastEvent>>

Init ==
  /\ activation = 1 /\ stage = "Unloaded" /\ loaded = "None"
  /\ diskActivation = 0 /\ diskNotice = "None" /\ ready = FALSE
  /\ clientActivation = 0 /\ clientNotice = "None" /\ connected = FALSE
  /\ lastEvent = "Init"

Load(outcome) ==
  /\ stage = "Unloaded" /\ outcome \in Outcomes
  /\ loaded' = outcome /\ stage' = "Loaded" /\ lastEvent' = "Load"
  /\ UNCHANGED <<activation, diskActivation, diskNotice, ready,
                  clientActivation, clientNotice, connected>>
PublishNotice ==
  /\ stage = "Loaded"
  /\ diskActivation' = activation /\ diskNotice' = loaded
  /\ stage' = "Published" /\ lastEvent' = "PublishNotice"
  /\ UNCHANGED <<activation, loaded, ready, clientActivation, clientNotice, connected>>
PublishFailure ==
  /\ stage = "Loaded" /\ stage' = "Failed" /\ lastEvent' = "PublishFailure"
  /\ UNCHANGED <<activation, loaded, diskActivation, diskNotice, ready,
                  clientActivation, clientNotice, connected>>
MakeReady ==
  /\ stage = "Published" /\ ready' = TRUE
  /\ stage' = "Ready" /\ lastEvent' = "MakeReady"
  /\ UNCHANGED <<activation, loaded, diskActivation, diskNotice,
                  clientActivation, clientNotice, connected>>
Connect ==
  /\ ready /\ diskActivation = activation
  /\ clientActivation' = activation /\ clientNotice' = diskNotice /\ connected' = TRUE
  /\ lastEvent' = "Connect"
  /\ UNCHANGED <<activation, stage, loaded, diskActivation, diskNotice, ready>>
ReadStale ==
  /\ ~ready /\ diskActivation # activation
  /\ clientActivation' = activation /\ clientNotice' = "None" /\ connected' = FALSE
  /\ lastEvent' = "ReadStale"
  /\ UNCHANGED <<activation, stage, loaded, diskActivation, diskNotice, ready>>
BeginNextActivation ==
  /\ activation < MaxActivation
  /\ activation' = activation + 1 /\ stage' = "Unloaded" /\ loaded' = "None"
  /\ ready' = FALSE /\ connected' = FALSE /\ clientActivation' = 0 /\ clientNotice' = "None"
  /\ lastEvent' = "BeginNextActivation"
  /\ UNCHANGED <<diskActivation, diskNotice>>

Next == (\E outcome \in Outcomes : Load(outcome)) \/ PublishNotice \/ PublishFailure
        \/ MakeReady \/ Connect \/ ReadStale \/ BeginNextActivation
Spec == Init /\ [][Next]_vars
TypeOK ==
  /\ activation \in 1..MaxActivation /\ diskActivation \in 0..MaxActivation
  /\ clientActivation \in 0..MaxActivation
  /\ stage \in {"Unloaded", "Loaded", "Published", "Ready", "Failed"}
  /\ loaded \in Outcomes /\ diskNotice \in Outcomes /\ clientNotice \in Outcomes
  /\ ready \in BOOLEAN /\ connected \in BOOLEAN
  /\ lastEvent \in {"Init", "Load", "PublishNotice", "PublishFailure", "MakeReady",
                     "Connect", "ReadStale", "BeginNextActivation"}
ReadyHasCurrentOutcome == ready => (diskActivation = activation /\ diskNotice = loaded)
VisibleRecovery == connected => (clientActivation = activation /\ clientNotice = loaded)
FailedNoticeNeverReady == stage = "Failed" => (~ready /\ ~connected)
StaleNeverDisplayed == (lastEvent = "ReadStale") => (~connected /\ clientNotice = "None")
=============================================================================
