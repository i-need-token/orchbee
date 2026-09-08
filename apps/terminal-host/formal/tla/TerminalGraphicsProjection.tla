--------------------- MODULE TerminalGraphicsProjection ---------------------
EXTENDS FiniteSets, Naturals, TLC

\* Refines runtime-model claims TH-006, TH-010, TH-011, TH-012, TH-016,
\* and TH-023. MutateImage, MutatePlacement, DeleteOrReset, StartAnimation,
\* AnimationDeadlineDue, AnimationTickChanged, AnimationTickStutter,
\* StopAnimation, StageLiveCapture, StageRenderHistoryCapture,
\* StageTextHistoryCapture, RejectBoundedOperation, ClearRejection,
\* AbortStaleCapture, CommitCapture, Acknowledge, StaleAcknowledge, and
\* InvalidateBase model the graphics transitions named by the package
\* contract. The model abstracts decoded bytes to one referenced resource and
\* one placement while retaining the ownership, generation, layer, atomicity,
\* history-detail, acknowledgement, retry, and animation-revision decisions.
\* RejectBoundedOperation starts after the concrete parser tests and graphics
\* corpus have classified a byte, dimension, or decoded-size limit violation;
\* numeric b-1/b/b+1 classification is deliberately outside this state model.
\* Acknowledge is the exact, post-validation output of
\* TerminalFrameBackpressure. StaleAcknowledge represents a rejected raw input
\* and must preserve the current graphics flight and acknowledged resource
\* base. The configured reader is the pointwise graphics projection; the
\* shared two-reader invalidation/flight interleaving remains in that model.

CONSTANTS
  Attachments,
  MaxRevision,
  MaxGraphicsGeneration,
  MaxResourceGeneration,
  MaxSequence

Layers == {"below-background", "below-text", "above-text"}
PlacementKinds == {"pin", "virtual", "relative"}
CaptureKinds == {"none", "live", "render-history", "text-history"}
NoLayer == "no-layer"
NoPlacement == "no-placement"
NoResourceGeneration == MaxResourceGeneration + 1

VARIABLES
  revision,
  graphicsGeneration,
  resourceGeneration,
  mutationCount,
  resourceMutationCount,
  animationChangeCount,
  resourcePresent,
  placementPresent,
  canonicalLayer,
  canonicalPlacementKind,
  animationRunning,
  animationDue,
  dirty,
  acknowledgedResourceGeneration,
  forceSnapshot,
  stagedKind,
  stagedRevision,
  stagedGraphicsGeneration,
  stagedResourceGeneration,
  stagedHasResource,
  stagedHasPlacement,
  stagedResolved,
  stagedLayer,
  flightKind,
  flightRevision,
  flightGraphicsGeneration,
  flightResourceGeneration,
  flightHasResource,
  flightHasPlacement,
  flightResolved,
  flightLayer,
  flightSequence,
  nextSequence,
  lastAcknowledgedSequence,
  staleAcknowledgementObserved,
  rejected,
  rejectedRevision,
  rejectedGraphicsGeneration,
  rejectedResourceGeneration

vars ==
  <<revision, graphicsGeneration, resourceGeneration, mutationCount,
    resourceMutationCount, animationChangeCount, resourcePresent,
    placementPresent, canonicalLayer, canonicalPlacementKind,
    animationRunning, animationDue, dirty,
    acknowledgedResourceGeneration, forceSnapshot, stagedKind,
    stagedRevision, stagedGraphicsGeneration, stagedResourceGeneration,
    stagedHasResource, stagedHasPlacement, stagedResolved, stagedLayer,
    flightKind, flightRevision, flightGraphicsGeneration,
    flightResourceGeneration, flightHasResource, flightHasPlacement,
    flightResolved, flightLayer, flightSequence, nextSequence,
    lastAcknowledgedSequence, staleAcknowledgementObserved, rejected,
    rejectedRevision, rejectedGraphicsGeneration, rejectedResourceGeneration>>

Init ==
  /\ revision = 0
  /\ graphicsGeneration = 0
  /\ resourceGeneration = 0
  /\ mutationCount = 0
  /\ resourceMutationCount = 0
  /\ animationChangeCount = 0
  /\ resourcePresent = FALSE
  /\ placementPresent = FALSE
  /\ canonicalLayer = NoLayer
  /\ canonicalPlacementKind = NoPlacement
  /\ animationRunning = FALSE
  /\ animationDue = FALSE
  /\ dirty = [attachment \in Attachments |-> TRUE]
  /\ acknowledgedResourceGeneration =
       [attachment \in Attachments |-> NoResourceGeneration]
  /\ forceSnapshot = [attachment \in Attachments |-> TRUE]
  /\ stagedKind = [attachment \in Attachments |-> "none"]
  /\ stagedRevision = [attachment \in Attachments |-> 0]
  /\ stagedGraphicsGeneration = [attachment \in Attachments |-> 0]
  /\ stagedResourceGeneration = [attachment \in Attachments |-> 0]
  /\ stagedHasResource = [attachment \in Attachments |-> FALSE]
  /\ stagedHasPlacement = [attachment \in Attachments |-> FALSE]
  /\ stagedResolved = [attachment \in Attachments |-> FALSE]
  /\ stagedLayer = [attachment \in Attachments |-> NoLayer]
  /\ flightKind = [attachment \in Attachments |-> "none"]
  /\ flightRevision = [attachment \in Attachments |-> 0]
  /\ flightGraphicsGeneration = [attachment \in Attachments |-> 0]
  /\ flightResourceGeneration = [attachment \in Attachments |-> 0]
  /\ flightHasResource = [attachment \in Attachments |-> FALSE]
  /\ flightHasPlacement = [attachment \in Attachments |-> FALSE]
  /\ flightResolved = [attachment \in Attachments |-> FALSE]
  /\ flightLayer = [attachment \in Attachments |-> NoLayer]
  /\ flightSequence = [attachment \in Attachments |-> 0]
  /\ nextSequence = [attachment \in Attachments |-> 1]
  /\ lastAcknowledgedSequence = [attachment \in Attachments |-> 0]
  /\ staleAcknowledgementObserved = FALSE
  /\ rejected = FALSE
  /\ rejectedRevision = 0
  /\ rejectedGraphicsGeneration = 0
  /\ rejectedResourceGeneration = 0

CanMutate ==
  /\ ~rejected
  /\ revision < MaxRevision
  /\ graphicsGeneration < MaxGraphicsGeneration

MutateImage(layer, placementKind) ==
  /\ layer \in Layers
  /\ placementKind \in PlacementKinds
  /\ CanMutate
  /\ resourceGeneration < MaxResourceGeneration
  /\ revision' = revision + 1
  /\ graphicsGeneration' = graphicsGeneration + 1
  /\ resourceGeneration' = resourceGeneration + 1
  /\ mutationCount' = mutationCount + 1
  /\ resourceMutationCount' = resourceMutationCount + 1
  /\ resourcePresent' = TRUE
  /\ placementPresent' = TRUE
  /\ canonicalLayer' = layer
  /\ canonicalPlacementKind' = placementKind
  /\ dirty' = [attachment \in Attachments |-> TRUE]
  /\ UNCHANGED <<animationChangeCount, animationRunning, animationDue,
                  acknowledgedResourceGeneration, forceSnapshot, stagedKind,
                  stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, stagedHasResource,
                  stagedHasPlacement, stagedResolved, stagedLayer, flightKind,
                  flightRevision, flightGraphicsGeneration,
                  flightResourceGeneration, flightHasResource,
                  flightHasPlacement, flightResolved, flightLayer,
                  flightSequence, nextSequence, lastAcknowledgedSequence,
                  staleAcknowledgementObserved, rejected, rejectedRevision,
                  rejectedGraphicsGeneration, rejectedResourceGeneration>>

MutatePlacement(layer, placementKind) ==
  /\ layer \in Layers
  /\ placementKind \in PlacementKinds
  /\ resourcePresent
  /\ CanMutate
  /\ revision' = revision + 1
  /\ graphicsGeneration' = graphicsGeneration + 1
  /\ mutationCount' = mutationCount + 1
  /\ placementPresent' = TRUE
  /\ canonicalLayer' = layer
  /\ canonicalPlacementKind' = placementKind
  /\ dirty' = [attachment \in Attachments |-> TRUE]
  /\ UNCHANGED <<resourceGeneration, resourceMutationCount,
                  animationChangeCount, resourcePresent, animationRunning,
                  animationDue, acknowledgedResourceGeneration,
                  forceSnapshot, stagedKind, stagedRevision,
                  stagedGraphicsGeneration, stagedResourceGeneration,
                  stagedHasResource, stagedHasPlacement, stagedResolved,
                  stagedLayer, flightKind, flightRevision,
                  flightGraphicsGeneration, flightResourceGeneration,
                  flightHasResource, flightHasPlacement, flightResolved,
                  flightLayer, flightSequence, nextSequence,
                  lastAcknowledgedSequence, staleAcknowledgementObserved,
                  rejected, rejectedRevision, rejectedGraphicsGeneration,
                  rejectedResourceGeneration>>

DeleteOrReset ==
  /\ resourcePresent
  /\ CanMutate
  /\ resourceGeneration < MaxResourceGeneration
  /\ revision' = revision + 1
  /\ graphicsGeneration' = graphicsGeneration + 1
  /\ resourceGeneration' = resourceGeneration + 1
  /\ mutationCount' = mutationCount + 1
  /\ resourceMutationCount' = resourceMutationCount + 1
  /\ resourcePresent' = FALSE
  /\ placementPresent' = FALSE
  /\ canonicalLayer' = NoLayer
  /\ canonicalPlacementKind' = NoPlacement
  /\ animationRunning' = FALSE
  /\ animationDue' = FALSE
  /\ dirty' = [attachment \in Attachments |-> TRUE]
  /\ acknowledgedResourceGeneration' =
       [attachment \in Attachments |-> NoResourceGeneration]
  /\ forceSnapshot' = [attachment \in Attachments |-> TRUE]
  /\ UNCHANGED <<animationChangeCount, stagedKind, stagedRevision,
                  stagedGraphicsGeneration, stagedResourceGeneration,
                  stagedHasResource, stagedHasPlacement, stagedResolved,
                  stagedLayer, flightKind, flightRevision,
                  flightGraphicsGeneration, flightResourceGeneration,
                  flightHasResource, flightHasPlacement, flightResolved,
                  flightLayer, flightSequence, nextSequence,
                  lastAcknowledgedSequence, staleAcknowledgementObserved,
                  rejected, rejectedRevision, rejectedGraphicsGeneration,
                  rejectedResourceGeneration>>

StartAnimation ==
  /\ resourcePresent
  /\ ~animationRunning
  /\ animationRunning' = TRUE
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationDue, dirty,
                  acknowledgedResourceGeneration, forceSnapshot, stagedKind,
                  stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, stagedHasResource,
                  stagedHasPlacement, stagedResolved, stagedLayer, flightKind,
                  flightRevision, flightGraphicsGeneration,
                  flightResourceGeneration, flightHasResource,
                  flightHasPlacement, flightResolved, flightLayer,
                  flightSequence, nextSequence, lastAcknowledgedSequence,
                  staleAcknowledgementObserved, rejected, rejectedRevision,
                  rejectedGraphicsGeneration, rejectedResourceGeneration>>

AnimationDeadlineDue ==
  /\ animationRunning
  /\ ~animationDue
  /\ animationDue' = TRUE
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationRunning, dirty,
                  acknowledgedResourceGeneration, forceSnapshot, stagedKind,
                  stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, stagedHasResource,
                  stagedHasPlacement, stagedResolved, stagedLayer, flightKind,
                  flightRevision, flightGraphicsGeneration,
                  flightResourceGeneration, flightHasResource,
                  flightHasPlacement, flightResolved, flightLayer,
                  flightSequence, nextSequence, lastAcknowledgedSequence,
                  staleAcknowledgementObserved, rejected, rejectedRevision,
                  rejectedGraphicsGeneration, rejectedResourceGeneration>>

AnimationTickChanged ==
  /\ animationRunning
  /\ animationDue
  /\ resourcePresent
  /\ CanMutate
  /\ resourceGeneration < MaxResourceGeneration
  /\ revision' = revision + 1
  /\ graphicsGeneration' = graphicsGeneration + 1
  /\ resourceGeneration' = resourceGeneration + 1
  /\ animationChangeCount' = animationChangeCount + 1
  /\ animationDue' = FALSE
  /\ dirty' = [attachment \in Attachments |-> TRUE]
  /\ UNCHANGED <<mutationCount, resourceMutationCount, resourcePresent,
                  placementPresent, canonicalLayer, canonicalPlacementKind,
                  animationRunning, acknowledgedResourceGeneration,
                  forceSnapshot, stagedKind, stagedRevision,
                  stagedGraphicsGeneration, stagedResourceGeneration,
                  stagedHasResource, stagedHasPlacement, stagedResolved,
                  stagedLayer, flightKind, flightRevision,
                  flightGraphicsGeneration, flightResourceGeneration,
                  flightHasResource, flightHasPlacement, flightResolved,
                  flightLayer, flightSequence, nextSequence,
                  lastAcknowledgedSequence, staleAcknowledgementObserved,
                  rejected, rejectedRevision, rejectedGraphicsGeneration,
                  rejectedResourceGeneration>>

AnimationTickStutter ==
  /\ animationRunning
  /\ animationDue
  /\ animationDue' = FALSE
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationRunning, dirty,
                  acknowledgedResourceGeneration, forceSnapshot, stagedKind,
                  stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, stagedHasResource,
                  stagedHasPlacement, stagedResolved, stagedLayer, flightKind,
                  flightRevision, flightGraphicsGeneration,
                  flightResourceGeneration, flightHasResource,
                  flightHasPlacement, flightResolved, flightLayer,
                  flightSequence, nextSequence, lastAcknowledgedSequence,
                  staleAcknowledgementObserved, rejected, rejectedRevision,
                  rejectedGraphicsGeneration, rejectedResourceGeneration>>

StopAnimation ==
  /\ animationRunning
  /\ animationRunning' = FALSE
  /\ animationDue' = FALSE
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, dirty,
                  acknowledgedResourceGeneration, forceSnapshot, stagedKind,
                  stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, stagedHasResource,
                  stagedHasPlacement, stagedResolved, stagedLayer, flightKind,
                  flightRevision, flightGraphicsGeneration,
                  flightResourceGeneration, flightHasResource,
                  flightHasPlacement, flightResolved, flightLayer,
                  flightSequence, nextSequence, lastAcknowledgedSequence,
                  staleAcknowledgementObserved, rejected, rejectedRevision,
                  rejectedGraphicsGeneration, rejectedResourceGeneration>>

CanStage(attachment) ==
  /\ attachment \in Attachments
  /\ dirty[attachment]
  /\ stagedKind[attachment] = "none"
  /\ flightKind[attachment] = "none"

StageCapture(attachment, kind, intersects) ==
  /\ CanStage(attachment)
  /\ kind \in {"live", "render-history", "text-history"}
  /\ intersects \in BOOLEAN
  /\ LET selectedPlacement ==
           placementPresent /\ (kind = "live" \/ (kind = "render-history" /\ intersects))
         includeResource ==
           selectedPlacement /\
             (kind = "render-history" \/ forceSnapshot[attachment] \/
               acknowledgedResourceGeneration[attachment] # resourceGeneration)
     IN
       /\ stagedKind' = [stagedKind EXCEPT ![attachment] = kind]
       /\ stagedRevision' = [stagedRevision EXCEPT ![attachment] = revision]
       /\ stagedGraphicsGeneration' =
            [stagedGraphicsGeneration EXCEPT ![attachment] = graphicsGeneration]
       /\ stagedResourceGeneration' =
            [stagedResourceGeneration EXCEPT ![attachment] = resourceGeneration]
       /\ stagedHasResource' =
            [stagedHasResource EXCEPT ![attachment] = includeResource]
       /\ stagedHasPlacement' =
            [stagedHasPlacement EXCEPT ![attachment] = selectedPlacement]
       /\ stagedResolved' =
            [stagedResolved EXCEPT ![attachment] = selectedPlacement]
       /\ stagedLayer' =
            [stagedLayer EXCEPT ![attachment] =
               IF selectedPlacement THEN canonicalLayer ELSE NoLayer]
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationRunning, animationDue,
                  dirty, acknowledgedResourceGeneration, forceSnapshot,
                  flightKind, flightRevision, flightGraphicsGeneration,
                  flightResourceGeneration, flightHasResource,
                  flightHasPlacement, flightResolved, flightLayer,
                  flightSequence, nextSequence, lastAcknowledgedSequence,
                  staleAcknowledgementObserved, rejected, rejectedRevision,
                  rejectedGraphicsGeneration, rejectedResourceGeneration>>

StageLiveCapture(attachment) == StageCapture(attachment, "live", TRUE)

StageRenderHistoryCapture(attachment, intersects) ==
  StageCapture(attachment, "render-history", intersects)

StageTextHistoryCapture(attachment) ==
  StageCapture(attachment, "text-history", FALSE)

RejectBoundedOperation ==
  /\ ~rejected
  /\ rejected' = TRUE
  /\ rejectedRevision' = revision
  /\ rejectedGraphicsGeneration' = graphicsGeneration
  /\ rejectedResourceGeneration' = resourceGeneration
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationRunning, animationDue,
                  dirty, acknowledgedResourceGeneration, forceSnapshot,
                  stagedKind, stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, stagedHasResource,
                  stagedHasPlacement, stagedResolved, stagedLayer, flightKind,
                  flightRevision, flightGraphicsGeneration,
                  flightResourceGeneration, flightHasResource,
                  flightHasPlacement, flightResolved, flightLayer,
                  flightSequence, nextSequence, lastAcknowledgedSequence,
                  staleAcknowledgementObserved>>

ClearRejection ==
  /\ rejected
  /\ rejected' = FALSE
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationRunning, animationDue,
                  dirty, acknowledgedResourceGeneration, forceSnapshot,
                  stagedKind, stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, stagedHasResource,
                  stagedHasPlacement, stagedResolved, stagedLayer, flightKind,
                  flightRevision, flightGraphicsGeneration,
                  flightResourceGeneration, flightHasResource,
                  flightHasPlacement, flightResolved, flightLayer,
                  flightSequence, nextSequence, lastAcknowledgedSequence,
                  staleAcknowledgementObserved, rejectedRevision,
                  rejectedGraphicsGeneration, rejectedResourceGeneration>>

AbortStaleCapture(attachment) ==
  /\ attachment \in Attachments
  /\ stagedKind[attachment] # "none"
  /\ \/ stagedRevision[attachment] # revision
     \/ stagedGraphicsGeneration[attachment] # graphicsGeneration
     \/ stagedResourceGeneration[attachment] # resourceGeneration
  /\ stagedKind' = [stagedKind EXCEPT ![attachment] = "none"]
  /\ stagedHasResource' = [stagedHasResource EXCEPT ![attachment] = FALSE]
  /\ stagedHasPlacement' = [stagedHasPlacement EXCEPT ![attachment] = FALSE]
  /\ stagedResolved' = [stagedResolved EXCEPT ![attachment] = FALSE]
  /\ stagedLayer' = [stagedLayer EXCEPT ![attachment] = NoLayer]
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationRunning, animationDue,
                  dirty, acknowledgedResourceGeneration, forceSnapshot,
                  stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, flightKind, flightRevision,
                  flightGraphicsGeneration, flightResourceGeneration,
                  flightHasResource, flightHasPlacement, flightResolved,
                  flightLayer, flightSequence, nextSequence,
                  lastAcknowledgedSequence, staleAcknowledgementObserved,
                  rejected, rejectedRevision, rejectedGraphicsGeneration,
                  rejectedResourceGeneration>>

CommitCapture(attachment) ==
  /\ attachment \in Attachments
  /\ stagedKind[attachment] # "none"
  /\ flightKind[attachment] = "none"
  /\ stagedRevision[attachment] = revision
  /\ stagedGraphicsGeneration[attachment] = graphicsGeneration
  /\ stagedResourceGeneration[attachment] = resourceGeneration
  /\ nextSequence[attachment] <= MaxSequence
  /\ flightKind' = [flightKind EXCEPT ![attachment] = stagedKind[attachment]]
  /\ flightRevision' = [flightRevision EXCEPT ![attachment] = stagedRevision[attachment]]
  /\ flightGraphicsGeneration' =
       [flightGraphicsGeneration EXCEPT ![attachment] = stagedGraphicsGeneration[attachment]]
  /\ flightResourceGeneration' =
       [flightResourceGeneration EXCEPT ![attachment] = stagedResourceGeneration[attachment]]
  /\ flightHasResource' =
       [flightHasResource EXCEPT ![attachment] = stagedHasResource[attachment]]
  /\ flightHasPlacement' =
       [flightHasPlacement EXCEPT ![attachment] = stagedHasPlacement[attachment]]
  /\ flightResolved' =
       [flightResolved EXCEPT ![attachment] = stagedResolved[attachment]]
  /\ flightLayer' = [flightLayer EXCEPT ![attachment] = stagedLayer[attachment]]
  /\ flightSequence' = [flightSequence EXCEPT ![attachment] = nextSequence[attachment]]
  /\ nextSequence' = [nextSequence EXCEPT ![attachment] = @ + 1]
  /\ stagedKind' = [stagedKind EXCEPT ![attachment] = "none"]
  /\ stagedHasResource' = [stagedHasResource EXCEPT ![attachment] = FALSE]
  /\ stagedHasPlacement' = [stagedHasPlacement EXCEPT ![attachment] = FALSE]
  /\ stagedResolved' = [stagedResolved EXCEPT ![attachment] = FALSE]
  /\ stagedLayer' = [stagedLayer EXCEPT ![attachment] = NoLayer]
  /\ dirty' = [dirty EXCEPT ![attachment] = FALSE]
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationRunning, animationDue,
                  acknowledgedResourceGeneration, forceSnapshot,
                  stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, lastAcknowledgedSequence,
                  staleAcknowledgementObserved, rejected, rejectedRevision,
                  rejectedGraphicsGeneration, rejectedResourceGeneration>>

Acknowledge(attachment) ==
  /\ attachment \in Attachments
  /\ flightKind[attachment] # "none"
  /\ LET live == flightKind[attachment] = "live"
     IN
       /\ acknowledgedResourceGeneration' =
            IF live
              THEN [acknowledgedResourceGeneration EXCEPT
                      ![attachment] = flightResourceGeneration[attachment]]
              ELSE acknowledgedResourceGeneration
       /\ forceSnapshot' =
            IF live
              THEN [forceSnapshot EXCEPT ![attachment] = FALSE]
              ELSE forceSnapshot
  /\ lastAcknowledgedSequence' =
       [lastAcknowledgedSequence EXCEPT ![attachment] = flightSequence[attachment]]
  /\ flightKind' = [flightKind EXCEPT ![attachment] = "none"]
  /\ flightHasResource' = [flightHasResource EXCEPT ![attachment] = FALSE]
  /\ flightHasPlacement' = [flightHasPlacement EXCEPT ![attachment] = FALSE]
  /\ flightResolved' = [flightResolved EXCEPT ![attachment] = FALSE]
  /\ flightLayer' = [flightLayer EXCEPT ![attachment] = NoLayer]
  /\ staleAcknowledgementObserved' = FALSE
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationRunning, animationDue,
                  dirty, stagedKind, stagedRevision,
                  stagedGraphicsGeneration, stagedResourceGeneration,
                  stagedHasResource, stagedHasPlacement, stagedResolved,
                  stagedLayer, flightRevision, flightGraphicsGeneration,
                  flightResourceGeneration, flightSequence, nextSequence,
                  rejected, rejectedRevision, rejectedGraphicsGeneration,
                  rejectedResourceGeneration>>

StaleAcknowledge(attachment, sequence) ==
  /\ attachment \in Attachments
  /\ sequence \in 0..MaxSequence
  /\ flightKind[attachment] # "none"
  /\ sequence # flightSequence[attachment]
  /\ ~staleAcknowledgementObserved
  /\ staleAcknowledgementObserved' = TRUE
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationRunning, animationDue,
                  dirty, acknowledgedResourceGeneration, forceSnapshot,
                  stagedKind, stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, stagedHasResource,
                  stagedHasPlacement, stagedResolved, stagedLayer, flightKind,
                  flightRevision, flightGraphicsGeneration,
                  flightResourceGeneration, flightHasResource,
                  flightHasPlacement, flightResolved, flightLayer,
                  flightSequence, nextSequence, lastAcknowledgedSequence,
                  rejected, rejectedRevision, rejectedGraphicsGeneration,
                  rejectedResourceGeneration>>

InvalidateBase(attachment) ==
  /\ attachment \in Attachments
  /\ \/ acknowledgedResourceGeneration[attachment] # NoResourceGeneration
     \/ ~forceSnapshot[attachment]
     \/ stagedKind[attachment] # "none"
     \/ flightKind[attachment] # "none"
  /\ acknowledgedResourceGeneration' =
       [acknowledgedResourceGeneration EXCEPT ![attachment] = NoResourceGeneration]
  /\ forceSnapshot' = [forceSnapshot EXCEPT ![attachment] = TRUE]
  /\ stagedKind' = [stagedKind EXCEPT ![attachment] = "none"]
  /\ stagedHasResource' = [stagedHasResource EXCEPT ![attachment] = FALSE]
  /\ stagedHasPlacement' = [stagedHasPlacement EXCEPT ![attachment] = FALSE]
  /\ stagedResolved' = [stagedResolved EXCEPT ![attachment] = FALSE]
  /\ stagedLayer' = [stagedLayer EXCEPT ![attachment] = NoLayer]
  /\ flightKind' = [flightKind EXCEPT ![attachment] = "none"]
  /\ flightHasResource' = [flightHasResource EXCEPT ![attachment] = FALSE]
  /\ flightHasPlacement' = [flightHasPlacement EXCEPT ![attachment] = FALSE]
  /\ flightResolved' = [flightResolved EXCEPT ![attachment] = FALSE]
  /\ flightLayer' = [flightLayer EXCEPT ![attachment] = NoLayer]
  /\ dirty' = [dirty EXCEPT ![attachment] = TRUE]
  /\ staleAcknowledgementObserved' = FALSE
  /\ UNCHANGED <<revision, graphicsGeneration, resourceGeneration,
                  mutationCount, resourceMutationCount, animationChangeCount,
                  resourcePresent, placementPresent, canonicalLayer,
                  canonicalPlacementKind, animationRunning, animationDue,
                  stagedRevision, stagedGraphicsGeneration,
                  stagedResourceGeneration, flightRevision,
                  flightGraphicsGeneration, flightResourceGeneration,
                  flightSequence, nextSequence, lastAcknowledgedSequence,
                  rejected, rejectedRevision, rejectedGraphicsGeneration,
                  rejectedResourceGeneration>>

AnimationTick == AnimationTickChanged \/ AnimationTickStutter

Next ==
  \/ \E layer \in Layers, placementKind \in PlacementKinds:
       MutateImage(layer, placementKind)
  \/ \E layer \in Layers, placementKind \in PlacementKinds:
       MutatePlacement(layer, placementKind)
  \/ DeleteOrReset
  \/ StartAnimation
  \/ AnimationDeadlineDue
  \/ AnimationTickChanged
  \/ AnimationTickStutter
  \/ StopAnimation
  \/ \E attachment \in Attachments: StageLiveCapture(attachment)
  \/ \E attachment \in Attachments, intersects \in BOOLEAN:
       StageRenderHistoryCapture(attachment, intersects)
  \/ \E attachment \in Attachments: StageTextHistoryCapture(attachment)
  \/ RejectBoundedOperation
  \/ ClearRejection
  \/ \E attachment \in Attachments: AbortStaleCapture(attachment)
  \/ \E attachment \in Attachments: CommitCapture(attachment)
  \/ \E attachment \in Attachments: Acknowledge(attachment)
  \/ \E attachment \in Attachments, sequence \in 0..MaxSequence:
       StaleAcknowledge(attachment, sequence)
  \/ \E attachment \in Attachments: InvalidateBase(attachment)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ WF_vars(AnimationTick)

TypeOK ==
  /\ revision \in 0..MaxRevision
  /\ graphicsGeneration \in 0..MaxGraphicsGeneration
  /\ resourceGeneration \in 0..MaxResourceGeneration
  /\ mutationCount \in 0..MaxRevision
  /\ resourceMutationCount \in 0..MaxResourceGeneration
  /\ animationChangeCount \in 0..MaxRevision
  /\ resourcePresent \in BOOLEAN
  /\ placementPresent \in BOOLEAN
  /\ canonicalLayer \in Layers \cup {NoLayer}
  /\ canonicalPlacementKind \in PlacementKinds \cup {NoPlacement}
  /\ animationRunning \in BOOLEAN
  /\ animationDue \in BOOLEAN
  /\ dirty \in [Attachments -> BOOLEAN]
  /\ acknowledgedResourceGeneration \in
       [Attachments -> 0..NoResourceGeneration]
  /\ forceSnapshot \in [Attachments -> BOOLEAN]
  /\ stagedKind \in [Attachments -> CaptureKinds]
  /\ stagedRevision \in [Attachments -> 0..MaxRevision]
  /\ stagedGraphicsGeneration \in [Attachments -> 0..MaxGraphicsGeneration]
  /\ stagedResourceGeneration \in [Attachments -> 0..MaxResourceGeneration]
  /\ stagedHasResource \in [Attachments -> BOOLEAN]
  /\ stagedHasPlacement \in [Attachments -> BOOLEAN]
  /\ stagedResolved \in [Attachments -> BOOLEAN]
  /\ stagedLayer \in [Attachments -> Layers \cup {NoLayer}]
  /\ flightKind \in [Attachments -> CaptureKinds]
  /\ flightRevision \in [Attachments -> 0..MaxRevision]
  /\ flightGraphicsGeneration \in [Attachments -> 0..MaxGraphicsGeneration]
  /\ flightResourceGeneration \in [Attachments -> 0..MaxResourceGeneration]
  /\ flightHasResource \in [Attachments -> BOOLEAN]
  /\ flightHasPlacement \in [Attachments -> BOOLEAN]
  /\ flightResolved \in [Attachments -> BOOLEAN]
  /\ flightLayer \in [Attachments -> Layers \cup {NoLayer}]
  /\ flightSequence \in [Attachments -> 0..MaxSequence]
  /\ nextSequence \in [Attachments -> 1..(MaxSequence + 1)]
  /\ lastAcknowledgedSequence \in [Attachments -> 0..MaxSequence]
  /\ staleAcknowledgementObserved \in BOOLEAN
  /\ rejected \in BOOLEAN
  /\ rejectedRevision \in 0..MaxRevision
  /\ rejectedGraphicsGeneration \in 0..MaxGraphicsGeneration
  /\ rejectedResourceGeneration \in 0..MaxResourceGeneration

CanonicalPlacementOwnsResource ==
  /\ placementPresent => resourcePresent
  /\ placementPresent => canonicalLayer \in Layers
  /\ placementPresent => canonicalPlacementKind \in PlacementKinds
  /\ ~placementPresent => canonicalLayer = NoLayer
  /\ ~placementPresent => canonicalPlacementKind = NoPlacement

RevisionAccountsForGraphicsChanges ==
  /\ revision = mutationCount + animationChangeCount
  /\ graphicsGeneration = mutationCount + animationChangeCount
  /\ resourceGeneration = resourceMutationCount + animationChangeCount

StagedProjectionIsResolved ==
  \A attachment \in Attachments:
    /\ stagedHasResource[attachment] => stagedHasPlacement[attachment]
    /\ stagedHasPlacement[attachment] => stagedResolved[attachment]
    /\ stagedHasPlacement[attachment] => stagedLayer[attachment] \in Layers
    /\ ~stagedHasPlacement[attachment] => ~stagedResolved[attachment]
    /\ ~stagedHasPlacement[attachment] => stagedLayer[attachment] = NoLayer

FlightProjectionIsResolved ==
  \A attachment \in Attachments:
    /\ flightHasResource[attachment] => flightHasPlacement[attachment]
    /\ flightHasPlacement[attachment] => flightResolved[attachment]
    /\ flightHasPlacement[attachment] => flightLayer[attachment] \in Layers
    /\ ~flightHasPlacement[attachment] => ~flightResolved[attachment]
    /\ ~flightHasPlacement[attachment] => flightLayer[attachment] = NoLayer

TextHistoryExcludesGraphics ==
  \A attachment \in Attachments:
    /\ stagedKind[attachment] = "text-history" =>
         ~stagedHasResource[attachment] /\ ~stagedHasPlacement[attachment]
    /\ flightKind[attachment] = "text-history" =>
         ~flightHasResource[attachment] /\ ~flightHasPlacement[attachment]

RenderHistoryIsComplete ==
  \A attachment \in Attachments:
    /\ stagedKind[attachment] = "render-history" =>
         stagedHasResource[attachment] = stagedHasPlacement[attachment]
    /\ flightKind[attachment] = "render-history" =>
         flightHasResource[attachment] = flightHasPlacement[attachment]

PlacementOnlyUsesExactAcknowledgedResource ==
  \A attachment \in Attachments:
    /\ stagedKind[attachment] = "live" /\
       stagedHasPlacement[attachment] /\ ~stagedHasResource[attachment] /\
       stagedResourceGeneration[attachment] = resourceGeneration =>
         /\ ~forceSnapshot[attachment]
         /\ acknowledgedResourceGeneration[attachment] =
              stagedResourceGeneration[attachment]
    /\ flightKind[attachment] = "live" /\
       flightHasPlacement[attachment] /\ ~flightHasResource[attachment] /\
       flightResourceGeneration[attachment] = resourceGeneration =>
         /\ ~forceSnapshot[attachment]
         /\ acknowledgedResourceGeneration[attachment] =
              flightResourceGeneration[attachment]

FlightNamesOneCommittedCapture ==
  \A attachment \in Attachments:
    flightKind[attachment] # "none" =>
      /\ flightRevision[attachment] <= revision
      /\ flightGraphicsGeneration[attachment] <= graphicsGeneration
      /\ flightResourceGeneration[attachment] <= resourceGeneration
      /\ flightSequence[attachment] < nextSequence[attachment]
      /\ flightSequence[attachment] > lastAcknowledgedSequence[attachment]

ForceSnapshotHasNoResourceBase ==
  \A attachment \in Attachments:
    forceSnapshot[attachment] =>
      acknowledgedResourceGeneration[attachment] = NoResourceGeneration

BoundedRejectionPreservesCanonical ==
  rejected =>
    /\ rejectedRevision = revision
    /\ rejectedGraphicsGeneration = graphicsGeneration
    /\ rejectedResourceGeneration = resourceGeneration

StaleAcknowledgementPreservesFlight ==
  [][\A attachment \in Attachments, sequence \in 0..MaxSequence:
       StaleAcknowledge(attachment, sequence) =>
         UNCHANGED <<flightKind, flightRevision,
                     flightGraphicsGeneration, flightResourceGeneration,
                     flightHasResource, flightHasPlacement, flightResolved,
                     flightLayer, flightSequence,
                     acknowledgedResourceGeneration,
                     lastAcknowledgedSequence, forceSnapshot>>]_vars

DueAnimationEventuallyTicks == animationDue ~> ~animationDue

=============================================================================
