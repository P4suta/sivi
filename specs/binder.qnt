// The slot binding automaton of `crates/sivi-core/src/binder.rs`, over one master of five slots.
// `crates/sivi/tests/binder_replay.rs` runs the traces of this model through `binder::step` and compares every state.
// The master mirrors a figure master with a section: the title, the subtitle, a text flow slot holding two blocks,
// an image slot that follows the text slot and holds one block, and a section slot holding two blocks.
module binder_model {
  // Whether a slot's capacity refuses a block, which the probe switches off.
  const CAPACITY: bool

  type Kind = Paragraph | ListBlock | Image | Code
  type Binding = Title | Subtitle | Section(int) | Flow
  type Phase = Start | Titled | Flowing | InSection(int)
  type Input = TitleHeading | SectionHeading | Block(Kind)

  pure val SLOTS = 0.to(4)
  pure val KINDS = Set(Paragraph, ListBlock, Image, Code)
  pure val NONE = -1
  // Where a count stops growing, which keeps the state finite; no trace reaches it.
  pure val SATURATION = 16

  pure def binds(s: int): Binding =
    if (s == 0) Title else if (s == 1) Subtitle else if (s == 4) Section(1) else Flow

  pure def accepts(s: int, k: Kind): bool =
    if (s == 0) false
    else if (s == 1) k == Paragraph
    else if (s == 2) k == Paragraph or k == ListBlock
    else if (s == 3) k == Image
    else k == Paragraph or k == ListBlock

  // The most blocks a slot holds, or NONE for no limit.
  pure def capacity(s: int): int =
    if (s == 2) 2 else if (s == 3) 1 else if (s == 4) 2 else NONE

  // The slot whose content this slot's content follows directly, or NONE.
  pure def follows(s: int): int = if (s == 3) 2 else NONE

  pure def sectionSlot(number: int): int =
    SLOTS.fold(NONE, (acc, s) => if (acc == NONE and binds(s) == Section(number)) s else acc)

  pure def firstSlot(candidates: Set[int]): int =
    candidates.fold(NONE, (acc, s) => if (acc == NONE or s < acc) s else acc)

  pure def hasRoom(s: int, count: int): bool =
    not(CAPACITY) or capacity(s) == NONE or count < capacity(s)

  pure def admits(s: int, count: int, k: Kind): bool = accepts(s, k) and hasRoom(s, count)

  // `slot_rules::choose`: the follower of the last slot when it is a flow slot that admits the block,
  // else the first flow slot in order that admits it; NONE when no flow slot accepts the kind or every one is full.
  pure def choose(count: int -> int, lastSlot: int, k: Kind): int = {
    val follower = firstSlot(SLOTS.filter(s => follows(s) == lastSlot))
    if (lastSlot != NONE and follower != NONE and binds(follower) == Flow and admits(follower, count.get(follower), k))
      follower
    else
      firstSlot(SLOTS.filter(s => binds(s) == Flow and admits(s, count.get(s), k)))
  }

  pure def anyFlowAccepts(k: Kind): bool = SLOTS.exists(s => binds(s) == Flow and accepts(s, k))

  var phase: Phase
  var filled: int -> int
  var last: int
  // What the last step read and decided: the slot it bound, or NONE with the code of the refusal.
  var input: Input
  var bound: int
  var refusal: str

  pure def grow(count: int -> int, s: int): int -> int =
    count.set(s, if (count.get(s) < SATURATION) count.get(s) + 1 else SATURATION)

  action init = all {
    phase' = Start,
    filled' = SLOTS.mapBy(_ => 0),
    last' = NONE,
    input' = TitleHeading,
    bound' = NONE,
    refusal' = "",
  }

  action bind(read: Input, s: int, after: Phase, fills: bool): bool = all {
    phase' = after,
    filled' = if (fills) grow(filled, s) else filled,
    last' = s,
    input' = read,
    bound' = s,
    refusal' = "",
  }

  action refuse(read: Input, code: str): bool = all {
    phase' = phase,
    filled' = filled,
    last' = last,
    input' = read,
    bound' = NONE,
    refusal' = code,
  }

  // `#`: only the first block of the slide, into the title slot.
  action readTitle =
    if (phase == Start) bind(TitleHeading, 0, Titled, true) else refuse(TitleHeading, "E0307")

  // `##`: the next section number, into its slot, which the heading labels without filling.
  action readSection = {
    val number = match phase { InSection(n) => n + 1 | _ => 1 }
    val s = sectionSlot(number)
    if (s == NONE) refuse(SectionHeading, "E0305") else bind(SectionHeading, s, InSection(number), false)
  }

  // A block inside a section goes to the section's slot or nowhere.
  action readInSection(number: int, k: Kind): bool = {
    val s = sectionSlot(number)
    if (s == NONE) refuse(Block(k), "E0305")
    else if (admits(s, filled.get(s), k)) bind(Block(k), s, InSection(number), true)
    else if (accepts(s, k)) refuse(Block(k), "E0306")
    else refuse(Block(k), "E0303")
  }

  // A block of the flow: the subtitle right after the title, else the slot the slot rules choose.
  action readFlow(k: Kind): bool =
    if (phase == Titled and k == Paragraph and filled.get(1) == 0)
      bind(Block(k), 1, Flowing, true)
    else {
      val s = choose(filled, last, k)
      if (s != NONE) bind(Block(k), s, Flowing, true)
      else if (anyFlowAccepts(k)) refuse(Block(k), "E0306")
      else refuse(Block(k), "E0303")
    }

  action readBlock(k: Kind): bool =
    match phase { InSection(n) => readInSection(n, k) | _ => readFlow(k) }

  action step = any {
    readTitle,
    readSection,
    nondet k = KINDS.oneOf()
    readBlock(k),
  }

  // Every slot holds at most its capacity.
  val capacityKept = SLOTS.forall(s => capacity(s) == NONE or filled.get(s) <= capacity(s))

  // The title and the subtitle bind at most once, and the title only as the first block.
  val headingsOnce = and {
    filled.get(0) <= 1,
    filled.get(1) <= 1,
    (bound == 0) implies (phase == Titled and SLOTS.forall(s => s == 0 or filled.get(s) == 0)),
  }

  // A bound block went to a slot that accepts its kind, except a section heading, which labels its slot.
  val boundAccepted = match input {
    | Block(k) => bound == NONE or accepts(bound, k)
    | SectionHeading => bound == NONE or binds(bound) == Section(1)
    | TitleHeading => bound == NONE or bound == 0
  }

  // A refusal names one of the binder's codes and leaves no slot bound.
  val refusalNamed = (refusal != "" implies bound == NONE)
    and (refusal == "" or Set("E0303", "E0305", "E0306", "E0307").contains(refusal))

  // Inside a section every bound block went to the section's slot.
  val sectionKept = match phase {
    | InSection(n) => bound == NONE or bound == sectionSlot(n)
    | _ => true
  }

  val safe = capacityKept and headingsOnce and boundAccepted and refusalNamed and sectionKept

  // The outcomes every simulation must reach.
  val boundFollower = bound == 3 and last == 3 and input == Block(Image)
  val refusedCapacity = refusal == "E0306"
  val refusedNoSlot = refusal == "E0303"
  val sectionBound = bound == 4
  val titleRefused = refusal == "E0307"
}

// The automaton as the binder implements it.
module binder {
  import binder_model(CAPACITY = true).*

  run figureTest = init
    .then(readTitle)
    .then(readBlock(Paragraph))
    .then(readBlock(Image))
    .then(readBlock(Paragraph))
    .then(readBlock(ListBlock))
    .then(readBlock(Image))
    .then(all {
      assert(filled.get(0) == 1 and filled.get(1) == 1 and filled.get(2) == 2 and filled.get(3) == 1),
      assert(refusal == "E0306"),
      init,
    })

  run sectionTest = init
    .then(readTitle)
    .then(readSection)
    .then(readBlock(ListBlock))
    .then(readBlock(Code))
    .then(readSection)
    .then(all {
      assert(phase == InSection(1) and filled.get(4) == 1 and refusal == "E0305"),
      init,
    })
}

// The automaton without capacities, which must break `safe`.
module binder_probe {
  import binder_model(CAPACITY = false).*

  // Without capacities a second image fills the image slot past its one block.
  run overfillTest = init
    .then(readTitle)
    .then(readBlock(Image))
    .then(readBlock(Image))
    .then(all {
      assert(filled.get(3) == 2 and not(capacityKept)),
      init,
    })
}
