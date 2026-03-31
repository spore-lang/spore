/// Fetch multiple URLs in parallel and return their bodies.
fn fetch_all(urls: List[Str]) -> List[Str] ! [NetError, Timeout]
    uses [NetRead, Spawn]
    cost ≤ urls.len * per_fetch_cost
{
    parallel_scope {
        urls |> map(|url| spawn { fetch(url) })
             |> map(|task| task.await?)
    }
}
