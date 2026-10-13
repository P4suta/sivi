//! A specification that claims the opposite of what the function computes, which the Verus gate must refuse.
//! Only the standalone Verus driver reads this crate, so it names the Verus prelude outright.
use vstd::prelude::*;

verus! {

/// Whether one more use fits below `limit`, specified upside down.
pub fn admits(limit: u32, used: u32) -> (fits: bool)
    ensures
        fits == (used >= limit),
{
    used < limit
}

} // verus!
