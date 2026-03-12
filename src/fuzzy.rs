#![allow(dead_code)]

#[derive(Debug, Clone, PartialEq)]
pub struct Entry<T> {
    pub data: T,
    pub text: String,
    pub text_lower: String,
    pub base_score: f64,
}

impl<T> Entry<T> {
    pub fn new(data: T, text: impl Into<String>, base_score: f64) -> Self {
        let text = text.into();
        Self {
            data,
            text_lower: text.to_lowercase(),
            text,
            base_score,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchResult<T> {
    pub data: T,
    pub positions: Vec<usize>,
    pub score: f64,
}

impl<T: Clone> MatchResult<T> {
    pub fn limit(results: &[Self], n: usize) -> Vec<Self> {
        results.iter().take(n).cloned().collect()
    }
}

pub const WORD_BOUNDARY_BONUS: f64 = 1.0;
pub const PROXIMITY_WEIGHT: f64 = 2.0;
pub const DENSITY_WEIGHT: f64 = 1.0;

pub const SQRT_TABLE: [f64; 65] = [
    1.0,
    1.414213562373095,
    1.732050807568877,
    2.0,
    2.23606797749979,
    2.449489742783178,
    2.645751311064591,
    2.82842712474619,
    3.0,
    3.16227766016838,
    3.3166247903554,
    3.464101615137754,
    3.605551275463989,
    3.741657386773941,
    3.872983346207417,
    4.0,
    4.123105625617661,
    4.242640687119285,
    4.358898943540674,
    4.47213595499958,
    4.58257569495584,
    4.69041575982343,
    4.795831523312719,
    4.898979485566356,
    5.0,
    5.099019513592784,
    5.196152422706632,
    5.291502622129181,
    5.385164807134504,
    5.477225575051661,
    5.567764362830022,
    5.656854249492381,
    5.744562646538029,
    5.830951894845301,
    5.916079783099616,
    6.0,
    6.082762530298219,
    6.164414002968976,
    6.244997998398398,
    6.324555320336759,
    6.403124237432849,
    6.48074069840786,
    6.557438524302,
    6.6332495807108,
    6.708203932499369,
    6.782329983125268,
    6.855654600401044,
    6.928203230275509,
    7.0,
    7.071067811865476,
    7.14142842854285,
    7.211102550927978,
    7.280109889280518,
    7.348469228349535,
    7.416198487095663,
    7.483314773547883,
    7.54983443527075,
    7.615773105863909,
    7.681145747868608,
    7.745966692414834,
    7.810249675906654,
    7.874007874011811,
    7.937253933193772,
    8.0,
    8.06225774829855,
];

pub fn fuzzy_match<T: Clone>(entries: &[Entry<T>], query: &str) -> Vec<MatchResult<T>> {
    let query_lower = query.to_lowercase();
    let query_chars: Vec<char> = query_lower.chars().collect();

    let mut results: Vec<MatchResult<T>> = entries
        .iter()
        .filter_map(|entry| {
            if query_chars.is_empty() {
                return Some(MatchResult {
                    data: entry.data.clone(),
                    positions: Vec::new(),
                    score: entry.base_score,
                });
            }

            let positions = find_subsequence_positions(&entry.text_lower, &query_chars)?;
            let word_bonus = positions
                .iter()
                .filter(|&&pos| is_word_boundary(&entry.text_lower, pos))
                .count() as f64
                * WORD_BOUNDARY_BONUS;

            let proximity_bonus = positions
                .windows(2)
                .map(|window| {
                    let gap = window[1] - window[0] - 1;
                    PROXIMITY_WEIGHT / SQRT_TABLE[gap.min(64)]
                })
                .sum::<f64>();

            let first = *positions.first()?;
            let last = *positions.last()?;
            let span = last - first;
            let density_multiplier =
                DENSITY_WEIGHT * (positions.len() as f64 / (span as f64 + 1.0));
            let length_penalty = entry.text.chars().count() as f64 * 0.01;
            let bonuses = word_bonus + proximity_bonus;

            let score =
                entry.base_score + bonuses + (density_multiplier * bonuses) - length_penalty;

            Some(MatchResult {
                data: entry.data.clone(),
                positions,
                score,
            })
        })
        .collect();

    results.sort_by(|a, b| b.score.total_cmp(&a.score));
    results
}

fn find_subsequence_positions(text: &str, query_chars: &[char]) -> Option<Vec<usize>> {
    if query_chars.is_empty() {
        return Some(Vec::new());
    }

    let chars: Vec<char> = text.chars().collect();
    let mut positions = Vec::with_capacity(query_chars.len());
    let mut search_from = 0;

    for query_char in query_chars {
        let mut found = None;
        for (idx, current) in chars.iter().enumerate().skip(search_from) {
            if current == query_char {
                found = Some(idx);
                break;
            }
        }

        let pos = found?;
        positions.push(pos);
        search_from = pos + 1;
    }

    Some(positions)
}

fn is_word_boundary(text: &str, pos: usize) -> bool {
    if pos == 0 {
        return true;
    }

    let chars: Vec<char> = text.chars().collect();
    chars
        .get(pos.wrapping_sub(1))
        .is_some_and(|prev| !prev.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::{fuzzy_match, Entry, MatchResult};

    #[test]
    fn exact_match_scores_higher_than_partial_match() {
        let entries = vec![
            Entry::new("partial", "2024-11-30-e-x-a-c-t-partial", 0.0),
            Entry::new("exact", "2024-12-01-exact-match", 0.0),
        ];

        let results = fuzzy_match(&entries, "exact");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].data, "exact");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn word_boundary_scores_higher_than_mid_word_match() {
        let entries = vec![
            Entry::new("boundary", "foo-bar", 0.0),
            Entry::new("midword", "foobar", 0.0),
        ];

        let results = fuzzy_match(&entries, "bar");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].data, "boundary");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn empty_query_returns_all_entries_by_base_score_descending() {
        let entries = vec![
            Entry::new("low", "alpha", 1.5),
            Entry::new("high", "beta", 4.0),
            Entry::new("mid", "gamma", 2.25),
        ];

        let results = fuzzy_match(&entries, "");

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].data, "high");
        assert_eq!(results[1].data, "mid");
        assert_eq!(results[2].data, "low");
    }

    #[test]
    fn no_match_returns_empty_vec() {
        let entries = vec![Entry::new("x", "workspace", 0.0)];
        let results = fuzzy_match(&entries, "zzz");
        assert!(results.is_empty());
    }

    #[test]
    fn limit_returns_top_n_results() {
        let entries = vec![
            Entry::new("first", "first-match", 3.0),
            Entry::new("second", "second-match", 2.0),
            Entry::new("third", "third-match", 1.0),
        ];
        let results = fuzzy_match(&entries, "m");

        let top_two = MatchResult::limit(&results, 2);
        assert_eq!(top_two.len(), 2);
        assert!(top_two[0].score >= top_two[1].score);
    }
}
