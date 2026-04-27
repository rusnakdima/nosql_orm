# Aggregation Pipeline

MongoDB-style aggregation stages.

---

## Stages

```rust
pub enum Stage {
    Match(Filter),
    Sort(OrderBy),
    Skip(u64),
    Limit(u64),
    Project(Projection),
    Group { _key: String, _accumulators: Vec<Accumulator> },
}
```

## Accumulator

```rust
pub enum Accumulator {
    Sum(String),
    Avg(String),
    Min(String),
    Max(String),
    First(String),
    Last(String),
    Push(String),
    AddToSet(String),
}
```

---

## AggregationPipeline

```rust
pub struct AggregationPipeline {
    stages: Vec<Stage>,
}
```

---

## Example

```rust
let results = repo.aggregate()
    .match(Filter::Eq("status", "active"))
    .group("category", vec![Accumulator::sum("count", "1")])
    .sort(OrderBy::desc("count"))
    .limit(10)
    .execute()
    .await?;