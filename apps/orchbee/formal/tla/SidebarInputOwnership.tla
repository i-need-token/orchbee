-------------------------- MODULE SidebarInputOwnership --------------------------
EXTENDS TLC

(* One symbolic key/paste fragment can span arbitrarily ordered reads, owner
   retirement and successful frame publication. Tree identity, menu commands,
   terminal mode writes and host decoding have their own refinement evidence. *)
VARIABLES owner, menuReturn, packetOwned, paintPending, fragment, bornOwned, leaked, nav
baseVars == <<owner, menuReturn, packetOwned, paintPending, fragment, bornOwned, leaked>>
vars == <<baseVars, nav>>
Owners == {"terminal", "sidebar", "navigator", "menu", "other"}
OwnsInput == owner # "terminal" \/ packetOwned \/ paintPending
CapturedFragment == IF fragment = "none" THEN "none" ELSE "captured"

Init ==
  /\ owner = "terminal" /\ menuReturn = "terminal"
  /\ packetOwned = FALSE /\ paintPending = FALSE
  /\ fragment = "none" /\ bornOwned = FALSE /\ leaked = FALSE
  /\ nav = [focus |-> "a", source |-> 0, painted |-> {}, paintFocus |-> "a",
             paintSource |-> 0, transferred |-> FALSE, release |-> FALSE,
             wrongTarget |-> FALSE]

EnterSidebar ==
  /\ owner = "terminal" /\ ~packetOwned /\ ~paintPending
  /\ owner' = "sidebar" /\ packetOwned' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<menuReturn, paintPending, bornOwned, leaked, nav>>

SidebarKey ==
  /\ owner = "sidebar" /\ ~packetOwned /\ ~paintPending
  /\ UNCHANGED vars

(* A validated keyboard adjustment or release changes local geometry, not
   the keyboard owner. Its packet and pending fragments wait for fresh paint. *)
ResizeSidebar ==
  /\ owner \in {"terminal", "sidebar"} /\ ~packetOwned /\ ~paintPending
  /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<owner, menuReturn, bornOwned, leaked, nav>>

OpenMenu ==
  /\ owner \in {"terminal", "sidebar"} /\ ~packetOwned /\ ~paintPending
  /\ menuReturn' = owner /\ owner' = "menu" /\ packetOwned' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<paintPending, bornOwned, leaked, nav>>

CloseMenu ==
  /\ owner = "menu"
  /\ owner' = menuReturn /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<menuReturn, bornOwned, leaked, nav>>

(* Explicit exit/activation and a hidden effective surface share retirement.
   If a menu covers the tree, the menu keeps capture but loses its return owner. *)
RetireSidebar ==
  /\ owner = "sidebar" \/ (owner = "menu" /\ menuReturn = "sidebar")
  /\ owner' = IF owner = "sidebar" THEN "terminal" ELSE "menu"
  /\ menuReturn' = "terminal" /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<bornOwned, leaked, nav>>

CoveringCapture ==
  /\ owner # "other"
  /\ owner' = "other" /\ menuReturn' = "terminal"
  /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<bornOwned, leaked, nav>>

EndCoveringCapture ==
  /\ owner = "other"
  /\ owner' = "terminal" /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ UNCHANGED <<menuReturn, bornOwned, leaked, nav>>

PublishFrame ==
  /\ paintPending /\ paintPending' = FALSE
  /\ \E drawn \in {{"a"}, {"b"}, {"a", "b"}} :
       nav' = IF owner = "navigator"
              THEN [nav EXCEPT !.painted = drawn, !.paintFocus = nav.focus,
                               !.paintSource = nav.source]
              ELSE [nav EXCEPT !.painted = {}]
  /\ UNCHANGED <<owner, menuReturn, packetOwned, fragment, bornOwned, leaked>>

PacketBoundary ==
  /\ packetOwned /\ packetOwned' = paintPending
  /\ UNCHANGED <<owner, menuReturn, paintPending, fragment, bornOwned, leaked, nav>>

BeginFragment ==
  /\ fragment = "none"
  /\ fragment' = IF packetOwned \/ paintPending THEN "captured" ELSE "raw"
  /\ bornOwned' = OwnsInput
  /\ UNCHANGED <<owner, menuReturn, packetOwned, paintPending, leaked, nav>>

FinishFragment ==
  /\ fragment # "none"
  /\ leaked' = leaked \/ (fragment = "raw" /\ ~OwnsInput /\ bornOwned)
  /\ fragment' = "none" /\ bornOwned' = FALSE
  /\ UNCHANGED <<owner, menuReturn, packetOwned, paintPending, nav>>

(* Navigator rows share an episode/source proof but retain distinct identities.
   Browsing/reconciliation cannot authorize a row before another complete paint.
   Ordinary PTY output is a stutter; navigation admission abstracts the existing
   Session operation after its independent busy and exact-source guards. *)
EnterNavigator ==
  /\ owner = "terminal" /\ ~packetOwned /\ ~paintPending
  /\ owner' = "navigator" /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ nav' = [nav EXCEPT !.painted = {}, !.transferred = FALSE]
  /\ UNCHANGED <<menuReturn, bornOwned, leaked>>

MoveNavigator ==
  /\ owner = "navigator" /\ ~packetOwned
  /\ nav' = [nav EXCEPT !.focus = IF nav.focus = "a" THEN "b" ELSE "a",
                        !.painted = {}]
  /\ paintPending' = TRUE
  /\ UNCHANGED <<owner, menuReturn, packetOwned, fragment, bornOwned, leaked>>

RefreshNavigator ==
  /\ owner = "navigator"
  /\ nav' = [nav EXCEPT !.source = IF nav.source = 0 THEN 1 ELSE 0, !.painted = {}]
  /\ paintPending' = TRUE
  /\ UNCHANGED <<owner, menuReturn, packetOwned, fragment, bornOwned, leaked>>

RetireNavigator(pointer) ==
  /\ owner = "navigator"
  /\ owner' = "terminal" /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ nav' = [nav EXCEPT !.painted = {}, !.release = @ \/ pointer]
  /\ UNCHANGED <<menuReturn, bornOwned, leaked>>

ActivateNavigator(target, pointer) ==
  /\ owner = "navigator" /\ ~packetOwned /\ ~paintPending
  /\ target \in nav.painted /\ nav.paintSource = nav.source
  /\ pointer \/ (target = nav.focus /\ nav.paintFocus = nav.focus)
  /\ ~nav.transferred
  /\ owner' = "terminal" /\ packetOwned' = TRUE /\ paintPending' = TRUE
  /\ fragment' = CapturedFragment
  /\ nav' = [nav EXCEPT !.painted = {}, !.transferred = TRUE,
                        !.release = @ \/ pointer,
                        !.wrongTarget = @ \/ target \notin nav.painted
                          \/ nav.paintSource # nav.source
                          \/ (~pointer /\ target # nav.focus)]
  /\ UNCHANGED <<menuReturn, bornOwned, leaked>>

EndNavigatorRelease ==
  /\ nav.release
  /\ nav' = [nav EXCEPT !.release = FALSE]
  /\ UNCHANGED baseVars

(* Each base action carries its own navigator stutter so TLC retains the named
   action headers required by the execution inventory. Conjoining UNCHANGED nav
   around their disjunction hides those headers while preserving the same graph. *)
BaseNext == EnterSidebar \/ SidebarKey \/ ResizeSidebar \/ OpenMenu \/ CloseMenu
            \/ RetireSidebar \/ CoveringCapture \/ EndCoveringCapture
            \/ PacketBoundary \/ BeginFragment \/ FinishFragment
Next == BaseNext \/ PublishFrame \/ EnterNavigator \/ MoveNavigator
        \/ RefreshNavigator \/ RetireNavigator(FALSE) \/ RetireNavigator(TRUE)
        \/ (\E target \in {"a", "b"}, pointer \in BOOLEAN : ActivateNavigator(target, pointer))
        \/ EndNavigatorRelease

TypeOK ==
  /\ owner \in Owners /\ menuReturn \in {"terminal", "sidebar"}
  /\ packetOwned \in BOOLEAN /\ paintPending \in BOOLEAN
  /\ fragment \in {"none", "raw", "captured"}
  /\ bornOwned \in BOOLEAN /\ leaked \in BOOLEAN
  /\ nav.focus \in {"a", "b"} /\ nav.paintFocus \in {"a", "b"}
  /\ nav.source \in {0, 1} /\ nav.paintSource \in {0, 1}
  /\ nav.painted \subseteq {"a", "b"}
  /\ nav.transferred \in BOOLEAN /\ nav.release \in BOOLEAN
  /\ nav.wrongTarget \in BOOLEAN
NoInputLeak == ~leaked
NoStaleFragment == fragment = "none" => ~bornOwned
ExactNavigatorTarget == ~nav.wrongTarget
FreshNavigatorPaint == nav.painted # {} => nav.paintSource = nav.source
Spec == Init /\ [][Next]_vars
==================================================================================
