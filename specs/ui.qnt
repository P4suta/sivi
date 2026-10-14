// The state machine of the terminal interface, `crates/sivi-core/src/ui_rules.rs`, over two slides and four pickers.
// `crates/sivi/tests/ui_replay.rs` runs the traces of this model through `ui_rules::step` and compares every state.
// The shell owns every effect; the machine decides which screen shows, whether a build runs, and what a key asks.
module ui_model {
  // Whether a running build refuses a second start, which the probe switches off.
  const GUARD: bool

  type Picker = Theme | Font | Palette | CodeTheme
  type Screen = Overview | Slides | Diagnostics | PickerScreen(Picker) | Help
  type Job = Idle | Building
  type Verdict = Passed | Failed
  type Action = Quit | Build | ToggleWatch | Next | Previous | ShowOverview | ShowSlides | ShowDiagnostics | ShowHelp
    | Pick(Picker) | Select | Back | OpenPdf
  type Event = Key(Action) | BuildDone(Verdict) | FileChanged | Tick
  type Message = BuildRunning | NothingToOpen | WatchOn | WatchOff
  type Chosen = DeckTheme | Carried(int)
  type Last = NoBuild | Ended(Verdict)
  type Effect = StartBuild(Chosen) | OpenPdfEffect | Install(int) | QuitEffect | Note(Message) | Nothing

  type St = {
    screen: Screen,
    job: Job,
    watch: bool,
    pending: bool,
    quitting: bool,
    slide: int,
    cursor: int,
    items: int,
    theme: Chosen,
    last: Last,
    tick: int,
  }

  // How many slides the deck has, how many phases the spinner turns through, and how many entries each picker lists.
  pure val SLIDES = 2
  pure val SPINNER = 4
  pure def listed(p: Picker): int = match p { Theme => 2 | Font => 2 | Palette => 1 | CodeTheme => 0 }

  pure val START: St = {
    screen: Overview, job: Idle, watch: false, pending: false, quitting: false,
    slide: 0, cursor: 0, items: 0, theme: DeckTheme, last: NoBuild, tick: 0,
  }

  pure val EVENTS = Set(
    Key(Quit), Key(Build), Key(ToggleWatch), Key(Next), Key(Previous),
    Key(ShowOverview), Key(ShowSlides), Key(ShowDiagnostics), Key(ShowHelp),
    Key(Pick(Theme)), Key(Pick(Font)), Key(Pick(Palette)), Key(Pick(CodeTheme)),
    Key(Select), Key(Back), Key(OpenPdf),
    BuildDone(Passed), BuildDone(Failed), FileChanged, Tick,
  )

  // The largest index a selection over `count` entries may take, which is zero for none.
  pure def lastIndex(count: int): int = if (count > 0) count - 1 else 0

  pure def clamp(s: St): St =
    s.with("slide", if (s.slide > lastIndex(SLIDES)) lastIndex(SLIDES) else s.slide)
     .with("cursor", if (s.cursor > lastIndex(s.items)) lastIndex(s.items) else s.cursor)

  // A build starts when none runs; otherwise the note says one runs.
  pure def start(s: St): (St, Effect) =
    if (s.job == Idle or not(GUARD))
      (s.with("job", Building).with("pending", false).with("tick", 0), StartBuild(s.theme))
    else (s, Note(BuildRunning))

  pure def isPicker(screen: Screen): bool = match screen { PickerScreen(_) => true | _ => false }

  pure def picked(s: St): (St, Effect) = match s.screen {
    | PickerScreen(p) => match p {
        | Theme => start(s.with("theme", Carried(s.cursor)).with("screen", Overview))
        | Font => (s, Install(s.cursor))
        | _ => (s.with("screen", Overview), Nothing)
      }
    | _ => (s, Nothing)
  }

  pure def key(s: St, a: Action): (St, Effect) = match a {
    | Quit => (s.with("quitting", true), QuitEffect)
    | Build => start(s)
    | ToggleWatch =>
      if (not(s.watch)) {
        val started = start(s.with("watch", true))
        (started._1, match started._2 { StartBuild(t) => StartBuild(t) | _ => Note(WatchOn) })
      } else (s.with("watch", false).with("pending", false), Note(WatchOff))
    | Next =>
      if (isPicker(s.screen)) (s.with("cursor", if (s.cursor < lastIndex(s.items)) s.cursor + 1 else s.cursor), Nothing)
      else if (s.screen == Help) (s, Nothing)
      else (s.with("slide", if (s.slide < lastIndex(SLIDES)) s.slide + 1 else s.slide), Nothing)
    | Previous =>
      if (isPicker(s.screen)) (s.with("cursor", if (s.cursor > 0) s.cursor - 1 else 0), Nothing)
      else if (s.screen == Help) (s, Nothing)
      else (s.with("slide", if (s.slide > 0) s.slide - 1 else 0), Nothing)
    | ShowOverview => (s.with("screen", Overview), Nothing)
    | ShowSlides => (s.with("screen", Slides), Nothing)
    | ShowDiagnostics => (s.with("screen", Diagnostics), Nothing)
    | ShowHelp => (s.with("screen", Help), Nothing)
    | Pick(p) => (s.with("screen", PickerScreen(p)).with("items", listed(p)).with("cursor", 0), Nothing)
    | Select => picked(s)
    | Back => (if (isPicker(s.screen) or s.screen == Help) s.with("screen", Overview) else s, Nothing)
    | OpenPdf => match s.last {
        | Ended(v) => if (v == Passed) (s, OpenPdfEffect) else (s, Note(NothingToOpen))
        | NoBuild => (s, Note(NothingToOpen))
      }
  }

  // One step of the machine: the state after `e` and the effect it asks of the shell.
  pure def advance(before: St, e: Event): (St, Effect) = {
    val s = clamp(before)
    val stepped = match e {
      | Key(a) => key(s, a)
      | BuildDone(v) => {
          val after = s.with("job", Idle).with("last", Ended(v)).with("tick", 0)
          if (after.pending and after.watch) start(after) else (after.with("pending", false), Nothing)
        }
      | FileChanged =>
          if (not(s.watch)) (s, Nothing)
          else if (s.job == Idle) start(s)
          else (s.with("pending", true), Nothing)
      | Tick => (s.with("tick", if (s.job == Building) (s.tick + 1) % SPINNER else 0), Nothing)
    }
    (clamp(stepped._1), stepped._2)
  }

  var st: St
  // What the last step read and left, and whether no build ran before it.
  var event: Event
  var effect: Effect
  var wasIdle: bool

  action init = all {
    st' = START,
    event' = Tick,
    effect' = Nothing,
    wasIdle' = true,
  }

  action apply(e: Event): bool = {
    val r = advance(st, e)
    all {
      wasIdle' = st.job == Idle,
      st' = r._1,
      event' = e,
      effect' = r._2,
    }
  }

  action step = {
    nondet e = EVENTS.oneOf()
    apply(e)
  }

  pure def isDone(e: Event): bool = match e { BuildDone(_) => true | _ => false }
  pure def isStart(x: Effect): bool = match x { StartBuild(_) => true | _ => false }

  // Quitting is one key away from every state.
  val quitSafe = {
    val q = advance(st, Key(Quit))
    q._1.quitting and q._2 == QuitEffect
  }

  // A build starts only when none ran, or when the one that ran has just ended, and leaves one running.
  val singleBuild = not(isStart(effect)) or ((wasIdle or isDone(event)) and st.job == Building)

  // Every selection stays within its count, and the spinner within its phases.
  val rangesKept = and {
    st.slide >= 0, st.slide <= lastIndex(SLIDES),
    st.cursor >= 0, st.cursor <= lastIndex(st.items),
    st.tick >= 0, st.tick < SPINNER,
  }

  val safe = quitSafe and singleBuild and rangesKept

  // The outcomes every simulation must reach.
  val themeBuilt = match effect { StartBuild(t) => t != DeckTheme | _ => false }
  val fontInstalled = match effect { Install(_) => true | _ => false }
  val pendingBuild = isDone(event) and isStart(effect)
  val quitWhileBuilding = st.quitting and st.job == Building
  val helpLeft = st.screen == Overview and event == Key(Back)
  val opened = effect == OpenPdfEffect
  val watchedOff = effect == Note(WatchOff)
}

// The machine as the interface implements it.
module ui {
  import ui_model(GUARD = true).*

  run watchTest = init
    .then(apply(Key(ToggleWatch)))
    .then(apply(FileChanged))
    .then(apply(BuildDone(Failed)))
    .then(apply(FileChanged))
    .then(apply(Key(Build)))
    .then(all {
      assert(st.job == Building and st.pending and effect == Note(BuildRunning)),
      init,
    })

  run pickTest = init
    .then(apply(Key(Pick(Theme))))
    .then(apply(Key(Next)))
    .then(apply(Key(Select)))
    .then(all {
      assert(st.theme == Carried(1) and st.screen == Overview and effect == StartBuild(Carried(1))),
      init,
    })
}

// The machine without the guard, which must break `safe`.
module ui_probe {
  import ui_model(GUARD = false).*

  // Without the guard a second key for a build starts another while one runs.
  run doubleBuildTest = init
    .then(apply(Key(Build)))
    .then(apply(Key(Build)))
    .then(all {
      assert(isStart(effect) and not(singleBuild)),
      init,
    })
}
