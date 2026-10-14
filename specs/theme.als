// The theme model: masters of slots with regions on a page, as `crates/sivi-theme/src/load.rs` validates them.
// The validator predicates are what the loader refuses: `slotsDisjoint` is E0505, `insidePage` is E0504,
// `sectionsContiguous` and `followsEnds` are E0508, and `oneTitle` is E0508.
// A slot lies in front with the content or at the back as a backdrop, and only front slots must stay apart,
// since the report attributes a laid-out item to a front region alone and leaves a backdrop's items to the backdrop.
// Each assertion holds under the validators and fails without its validator, which the unchecked commands show.
module theme

// A master, its slots, and what each slot binds.
sig Master {
  slots: set Slot
}

sig Slot {
  region: one Region,
  binds: one Binding,
  follows: lone Slot,
  layer: one Layer
}

// A region as its edges on a page of fixed size.
sig Region {
  left: one Int,
  top: one Int,
  right: one Int,
  bottom: one Int
}

abstract sig Binding {}
one sig Title, Subtitle, Flow extends Binding {}
sig Section extends Binding {
  index: one Int
}

// Where a slot lies: in front with the content, or at the back as a backdrop other slots may cover.
abstract sig Layer {}
one sig Front, Back extends Layer {}

// The page runs from zero to the page edge on each axis.
fun pageEdge: Int { 7 }

// A point at integer coordinates, which the attribution rules assign to a region.
sig Point {
  x: one Int,
  y: one Int
}

fact EverySlotHasOneMaster {
  all s: Slot | one m: Master | s in m.slots
}

fact RegionsHaveArea {
  all r: Region | r.left < r.right and r.top < r.bottom
}

pred contains[r: Region, p: Point] {
  r.left <= p.x and p.x <= r.right and r.top <= p.y and p.y <= r.bottom
}

pred overlaps[a, b: Region] {
  a.left < b.right and b.left < a.right and a.top < b.bottom and b.top < a.bottom
}

// The front slots of a master, whose regions the report attributes items to.
fun front[m: Master]: set Slot { { s: m.slots | s.layer = Front } }

// E0505: no two front slots of a master share interior points; a backdrop may lie under anything.
pred slotsDisjoint {
  all m: Master, disj a, b: front[m] | not overlaps[a.region, b.region]
}

// E0504: every region lies on the page.
pred insidePage {
  all r: Slot.region | r.left >= 0 and r.top >= 0 and r.right <= pageEdge and r.bottom <= pageEdge
}

// E0508: the section slots of a master are numbered one to their count, each once.
pred sectionsContiguous {
  all m: Master |
    let numbers = (m.slots.binds & Section).index |
      numbers = { i: Int | 1 <= i and i <= #(m.slots.binds & Section) }
      and (all i: numbers | one s: m.slots | s.binds in Section and s.binds.index = i)
}

// E0508: a slot follows another slot of its master, and no chain of follows loops.
pred followsEnds {
  all m: Master, s: m.slots | s.follows in m.slots - s
  no s: Slot | s in s.^follows
}

// E0508: a master binds its title and its subtitle once.
pred oneTitle {
  all m: Master | lone s: m.slots | s.binds = Title
  all m: Master | lone s: m.slots | s.binds = Subtitle
}

pred valid {
  slotsDisjoint and insidePage and sectionsContiguous and followsEnds and oneTitle
}

// `attribute_rules::region_of` names the first front region holding a point, which under the validators is the
// only front region whose interior holds it.
pred pointInAtMostOneInterior {
  all m: Master, p: Point | lone s: front[m] |
    s.region.left < p.x and p.x < s.region.right and s.region.top < p.y and p.y < s.region.bottom
}

// The binder maps the k-th `##` of a slide to exactly one slot of the master.
pred everySectionHasOneSlot {
  all m: Master, k: (m.slots.binds & Section).index | one s: m.slots | s.binds in Section and s.binds.index = k
}

// Following `follows` from any slot reaches a slot without a successor, inside the same master.
pred followsReachesAnEnd {
  all m: Master, s: m.slots | some e: s.*follows | no e.follows and e in m.slots
}

assert AttributionIsUnique {
  valid implies pointInAtMostOneInterior
}

assert SectionsAreHandles {
  valid implies everySectionHasOneSlot
}

assert FollowsChainsEnd {
  valid implies followsReachesAnEnd
}

// The assertions without their validator, which must each find a counterexample.
assert AttributionUnchecked {
  (insidePage and sectionsContiguous and followsEnds and oneTitle) implies pointInAtMostOneInterior
}

assert SectionsUnchecked {
  (slotsDisjoint and insidePage and followsEnds and oneTitle) implies everySectionHasOneSlot
}

assert FollowsUnchecked {
  (slotsDisjoint and insidePage and sectionsContiguous and oneTitle) implies followsReachesAnEnd
}

// A theme like the bundled one: a master with a title, two sections, a slot that follows another, and a backdrop
// under a front slot.
pred Populated {
  valid
  some m: Master | #m.slots >= 4
    and some s: m.slots | s.binds = Title
    and some disj a, b: m.slots | a.binds in Section and b.binds in Section
    and some s: m.slots | some s.follows
  some m: Master | some b: m.slots, f: front[m] | b.layer = Back and overlaps[b.region, f.region]
}

check AttributionIsUnique for 4 but 4 Int, 6 Binding
check SectionsAreHandles for 4 but 4 Int, 6 Binding
check FollowsChainsEnd for 4 but 4 Int, 6 Binding
check AttributionUnchecked for 4 but 4 Int, 6 Binding
check SectionsUnchecked for 4 but 4 Int, 6 Binding
check FollowsUnchecked for 4 but 4 Int, 6 Binding
run Populated for 4 but 4 Int, 6 Binding
