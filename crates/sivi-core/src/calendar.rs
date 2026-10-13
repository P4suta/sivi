//! Civil dates from Unix time, as the creation date of a PDF comes from `SOURCE_DATE_EPOCH` and never from a clock.

/// A date and time in the proleptic Gregorian calendar, in UTC.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Civil {
    pub year: i64,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

const SECONDS_PER_MINUTE: i64 = 60;
const MINUTES_PER_HOUR: i64 = 60;
const HOURS_PER_DAY: i64 = 24;
const SECONDS_PER_DAY: i64 = SECONDS_PER_MINUTE * MINUTES_PER_HOUR * HOURS_PER_DAY;

/// The last second a PDF date can hold, `9999-12-31T23:59:59Z`.
pub const LAST_SECOND: i64 = 253_402_300_799;

/// The days from `0000-03-01` to `1970-01-01`, which shifts the epoch to the start of a 400-year era.
const EPOCH_SHIFT: i64 = 719_468;
/// The days in a 400-year era.
const DAYS_PER_ERA: i64 = 146_097;
/// The days in four years less the leap day.
const DAYS_BEFORE_FOUR_YEAR_LEAP: i64 = 1460;
/// The days in a century less its missing leap day.
const DAYS_PER_CENTURY: i64 = 36_524;
/// The days in an era less its final leap day.
const DAYS_BEFORE_ERA_LEAP: i64 = 146_096;
const DAYS_PER_YEAR: i64 = 365;
const YEARS_PER_ERA: i64 = 400;
/// The length of the five-month cycle the month formula uses.
const DAYS_PER_FIVE_MONTHS: i64 = 153;
const MONTH_FORMULA_OFFSET: i64 = 2;
const MONTH_FORMULA_SCALE: i64 = 5;
/// Months counted from March, past which the civil year advances.
const MONTHS_FROM_MARCH_IN_YEAR: i64 = 10;
const MARCH: i64 = 3;
/// Months counted from March that fall after December, which subtract this to land in the next year.
const MONTHS_PAST_DECEMBER: i64 = 9;
const FEBRUARY: i64 = 2;
const FOUR: i64 = 4;
const HUNDRED: i64 = 100;

/// The civil date of `days` since `1970-01-01`, by the era algorithm.
fn civil_from_days(days: i64) -> Option<(i64, u8, u8)> {
    let shifted = days.checked_add(EPOCH_SHIFT)?;
    let era = shifted.div_euclid(DAYS_PER_ERA);
    let day_of_era = shifted.rem_euclid(DAYS_PER_ERA);
    let year_of_era = day_of_era
        .checked_sub(day_of_era.checked_div(DAYS_BEFORE_FOUR_YEAR_LEAP)?)?
        .checked_add(day_of_era.checked_div(DAYS_PER_CENTURY)?)?
        .checked_sub(day_of_era.checked_div(DAYS_BEFORE_ERA_LEAP)?)?
        .checked_div(DAYS_PER_YEAR)?;
    let year = year_of_era.checked_add(era.checked_mul(YEARS_PER_ERA)?)?;
    let day_of_year = day_of_era.checked_sub(
        DAYS_PER_YEAR
            .checked_mul(year_of_era)?
            .checked_add(year_of_era.checked_div(FOUR)?)?
            .checked_sub(year_of_era.checked_div(HUNDRED)?)?,
    )?;
    let month_from_march = MONTH_FORMULA_SCALE
        .checked_mul(day_of_year)?
        .checked_add(MONTH_FORMULA_OFFSET)?
        .checked_div(DAYS_PER_FIVE_MONTHS)?;
    let day = day_of_year
        .checked_sub(
            DAYS_PER_FIVE_MONTHS
                .checked_mul(month_from_march)?
                .checked_add(MONTH_FORMULA_OFFSET)?
                .checked_div(MONTH_FORMULA_SCALE)?,
        )?
        .checked_add(1)?;
    let month = if month_from_march < MONTHS_FROM_MARCH_IN_YEAR {
        month_from_march.checked_add(MARCH)?
    } else {
        month_from_march.checked_sub(MONTHS_PAST_DECEMBER)?
    };
    let year = if month <= FEBRUARY {
        year.checked_add(1)?
    } else {
        year
    };
    Some((year, u8::try_from(month).ok()?, u8::try_from(day).ok()?))
}

/// The civil date and time of `seconds` since the Unix epoch, or `None` before the epoch or past [`LAST_SECOND`].
#[must_use]
pub fn civil(seconds: i64) -> Option<Civil> {
    if !(0..=LAST_SECOND).contains(&seconds) {
        return None;
    }
    let days = seconds.div_euclid(SECONDS_PER_DAY);
    let of_day = seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days)?;
    Some(Civil {
        year,
        month,
        day,
        hour: u8::try_from(of_day.checked_div(SECONDS_PER_MINUTE * MINUTES_PER_HOUR)?).ok()?,
        minute: u8::try_from(
            of_day
                .checked_div(SECONDS_PER_MINUTE)?
                .rem_euclid(MINUTES_PER_HOUR),
        )
        .ok()?,
        second: u8::try_from(of_day.rem_euclid(SECONDS_PER_MINUTE)).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::{Civil, LAST_SECOND, civil};

    const fn at(year: i64, month: u8, day: u8, time: (u8, u8, u8)) -> Civil {
        let (hour, minute, second) = time;
        Civil {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    #[test]
    fn known_instants_map_to_their_civil_dates() {
        assert_eq!(civil(0), Some(at(1970, 1, 1, (0, 0, 0))));
        assert_eq!(civil(951_782_400), Some(at(2000, 2, 29, (0, 0, 0))));
        assert_eq!(civil(1_000_000_000), Some(at(2001, 9, 9, (1, 46, 40))));
        assert_eq!(civil(2_147_483_647), Some(at(2038, 1, 19, (3, 14, 7))));
        assert_eq!(civil(1_767_225_599), Some(at(2025, 12, 31, (23, 59, 59))));
        assert_eq!(civil(LAST_SECOND), Some(at(9999, 12, 31, (23, 59, 59))));
    }

    #[test]
    fn instants_outside_the_pdf_range_have_no_date() {
        assert_eq!(civil(-1), None);
        assert_eq!(civil(LAST_SECOND + 1), None);
        assert_eq!(civil(i64::MAX), None);
    }
}
