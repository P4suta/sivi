% The slot binding grammar as an executable reference, written apart from `crates/sivi-core/src/binder.rs`.
% `cargo xtask proofs --only prolog` binds the same slides with both and refuses any difference.
%
% The input is one JSON document: the slots of a master and the blocks of a slide.
% The output is one JSON document: the slot or the refusal of each block, and the first required slot left empty.
:- use_module(library(json)).
:- use_module(library(lists)).
:- use_module(library(apply)).

main :-
    current_prolog_flag(argv, [Path|_]),
    setup_call_cleanup(open(Path, read, In), json_read_dict(In, Input), close(In)),
    bind_all(Input.slots, Input.blocks, Steps, Final),
    unfilled(Input.slots, Final, Unfilled),
    json_write_dict(current_output, _{steps: Steps, unfilled: Unfilled}, [width(0)]),
    nl.

% The automaton starts before any block, with every slot empty and no last slot.
initial(Slots, state(start, Filled, -1)) :-
    length(Slots, Count),
    length(Filled, Count),
    maplist(=(0), Filled).

bind_all(Slots, Blocks, Steps, Final) :-
    initial(Slots, Start),
    foldl(bind_one(Slots), Blocks, Start-[], Final-Reversed),
    reverse(Reversed, Steps).

bind_one(Slots, Block, State-Done, Next-[Outcome|Done]) :-
    step(Slots, State, Block, Outcome, Next).

% A heading of a level, or a block with its kind and measures.
heading(Block, Level) :- get_dict(heading, Block, Level).

% `#` binds the title slot as the first block of the slide, and nothing else.
step(Slots, state(start, Filled, _), Block, _{slot: Index}, state(titled, Filled1, Index)) :-
    heading(Block, "title"),
    slot_binding(Slots, "title", Index),
    !,
    fill(Filled, Index, Filled1).
step(_, State, Block, _{refused: "E0307"}, State) :-
    heading(Block, "title"),
    !.

% `##` opens the next section, whose slot it labels without filling.
step(Slots, state(Phase, Filled, _), Block, _{slot: Index}, state(section(Number), Filled, Index)) :-
    heading(Block, "section"),
    next_section(Phase, Number),
    slot_binding(Slots, _{section: Number}, Index),
    !.
step(_, State, Block, _{refused: "E0305"}, State) :-
    heading(Block, "section"),
    !.

% A block inside a section goes to the section's slot or nowhere.
step(Slots, state(section(Number), Filled, Last), Block, Outcome, Next) :-
    !,
    in_section(Slots, Number, state(section(Number), Filled, Last), Block, Outcome, Next).

% A paragraph right after the title is the subtitle, when the master has a subtitle slot.
step(Slots, state(titled, Filled, _), Block, _{slot: Index}, state(flowing, Filled1, Index)) :-
    get_dict(kind, Block, "paragraph"),
    slot_binding(Slots, "subtitle", Index),
    nth0(Index, Filled, 0),
    !,
    fill(Filled, Index, Filled1).

% Any other block of the flow goes where the slot rules choose.
step(Slots, state(_, Filled, Last), Block, _{slot: Index}, state(flowing, Filled1, Index)) :-
    choose(Slots, Filled, Last, Block, Index),
    !,
    fill(Filled, Index, Filled1).
step(Slots, State, Block, _{refused: Code}, State) :-
    (   flow_slot_accepts(Slots, Block)
    ->  Code = "E0306"
    ;   Code = "E0303"
    ).

next_section(section(Current), Number) :- !, Number is Current + 1.
next_section(_, 1).

in_section(Slots, Number, state(Phase, Filled, _), Block, _{slot: Index}, state(Phase, Filled1, Index)) :-
    slot_binding(Slots, _{section: Number}, Index),
    nth0(Index, Slots, Slot),
    nth0(Index, Filled, Count),
    admits(Slot, Count, Block),
    !,
    fill(Filled, Index, Filled1).
in_section(Slots, Number, State, Block, _{refused: Code}, State) :-
    (   slot_binding(Slots, _{section: Number}, Index)
    ->  nth0(Index, Slots, Slot),
        (   accepts(Slot, Block)
        ->  Code = "E0306"
        ;   Code = "E0303"
        )
    ;   Code = "E0305"
    ).

% The first slot that binds `Binding`.
slot_binding(Slots, Binding, Index) :-
    nth0(Index, Slots, Slot),
    Slot.binds = Binding,
    !.

fill(Filled, Index, Filled1) :-
    nth0(Index, Filled, Count),
    Count1 is Count + 1,
    replace_nth(Index, Filled, Count1, Filled1).

replace_nth(0, [_|Rest], Value, [Value|Rest]) :- !.
replace_nth(Index, [Head|Rest], Value, [Head|Rest1]) :-
    Index1 is Index - 1,
    replace_nth(Index1, Rest, Value, Rest1).

accepts(Slot, Block) :-
    get_dict(kind, Block, Kind),
    memberchk(Kind, Slot.accepts).

% A slot admits a block when it accepts the kind, has room for one more block, and every measure stays within its capacity.
admits(Slot, Count, Block) :-
    accepts(Slot, Block),
    Capacity = Slot.capacity,
    room(Capacity.blocks, Count),
    within(Block.items, Capacity.items),
    within(Block.depth, Capacity.depth),
    within(Block.rows, Capacity.rows),
    within(Block.columns, Capacity.columns),
    within(Block.lines, Capacity.lines).

room(null, _) :- !.
room(Limit, Count) :- Count < Limit.

within(_, null) :- !.
within(Measure, Limit) :- Measure =< Limit.

% The follower of the last slot when it is a flow slot that admits the block, else the first flow slot in order that admits it.
choose(Slots, Filled, Last, Block, Index) :-
    Last >= 0,
    nth0(Index, Slots, Slot),
    Slot.follows == Last,
    !,
    Slot.binds == "flow",
    nth0(Index, Filled, Count),
    admits(Slot, Count, Block).
choose(Slots, Filled, _, Block, Index) :-
    nth0(Index, Slots, Slot),
    Slot.binds == "flow",
    nth0(Index, Filled, Count),
    admits(Slot, Count, Block),
    !.

flow_slot_accepts(Slots, Block) :-
    member(Slot, Slots),
    Slot.binds == "flow",
    accepts(Slot, Block),
    !.

% The first required slot left empty, or null.
unfilled(Slots, state(_, Filled, _), Index) :-
    nth0(Index, Slots, Slot),
    Slot.required == true,
    nth0(Index, Filled, 0),
    !.
unfilled(_, _, null).
