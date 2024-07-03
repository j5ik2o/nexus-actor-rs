use std::time::{Duration, Instant};
use crate::actor::actor::restart_statistics::RestartStatistics;
use crate::actor::supervisor::strategy_one_for_one::OneForOneStrategy;

#[tokio::test]
async fn test_one_for_one_strategy_request_restart_permission() {
    let cases = vec![
        (
            "no restart if max retries is 0",
            OneForOneStrategy::new(0, Duration::from_secs(0), None),
            RestartStatistics::new(),
            true,
            0,
        ),
        (
            "restart when duration is 0",
            OneForOneStrategy::new(1, Duration::from_secs(0), None),
            RestartStatistics::new(),
            false,
            1,
        ),
        (
            "no restart when duration is 0 and exceeds max retries",
            OneForOneStrategy::new(1, Duration::from_secs(0), None),
            RestartStatistics::with_values(vec![Instant::now() - Duration::from_secs(1)]),
            true,
            0,
        ),
        (
            "restart when duration set and within window",
            OneForOneStrategy::new(2, Duration::from_secs(10), None),
            RestartStatistics::with_values(vec![Instant::now() - Duration::from_secs(5)]),
            false,
            2,
        ),
        (
            "no restart when duration set, within window and exceeds max retries",
            OneForOneStrategy::new(1, Duration::from_secs(10), None),
            RestartStatistics::with_values(vec![
                Instant::now() - Duration::from_secs(5),
                Instant::now() - Duration::from_secs(5),
            ]),
            true,
            0,
        ),
        (
            "restart and FailureCount reset when duration set and outside window",
            OneForOneStrategy::new(1, Duration::from_secs(10), None),
            RestartStatistics::with_values(vec![
                Instant::now() - Duration::from_secs(11),
                Instant::now() - Duration::from_secs(11),
            ]),
            false,
            1,
        ),
    ];

    for (name, s, mut rs, expected_result, expected_count) in cases {
        let actual = s.should_stop(&mut rs).await;
        assert_eq!(actual, expected_result, "{}", name);
        assert_eq!(rs.number_of_failures(s.within_duration).await, expected_count, "{}", name);
    }
}
