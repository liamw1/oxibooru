# Benchmarks
These benchmarks are meant to give a rough idea of the relative speed 
differences between szurubooru and oxibooru for various operations. My 
mythodology here is very crude (often just taking a few measurements) and 
there's a lot of external factors that could affect the results, so take them 
with a grain of salt.

These benchmarks were performed on the same database consisting of about 125k 
images and 25k tags. Everything is containerized with Docker and all requests 
are made anonymously, so time to authenticate isn't included.

1. [Startup](#startup)
2. [Listing Posts](#listing-posts)
3. [Listing Tags](#listing-tags)
4. [Reverse Image Search](#reverse-image-search)

## Startup
- Here I measure the time it takes for the first "info" request, which gathers
  statistics about the size of the database.

    **`GET http://localhost:8080/api/info`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      1561 |
    | oxibooru          |         3 |

    Oxibooru blows Szurubooru out of the water here. The reason for this is
    that Szurubooru calculates the disk usage by iterating over the entire data
    directory, summing up the sizes of all the files it contains. This process 
    is quite slow for directories with many files. This request is the main
    reason that Szurubooru is unresponsive for so long after a cold start. In
    my experience it can take 10 seconds or more to become usable, much longer
    than the time of a single info request. That's because the server actually
    makes _two_ info request on startup, and it seems like they interfere with
    each other in a way that makes them slower than if they were run
    sequentially.

    Oxibooru avoids this by storing a running total of the disk usage inside 
    the database, which can be retrieved almost instantly. This makes cold
    starts lightning fast.
    
## Listing Posts
- Let's start with the simplest case: viewing the first page of posts with no
  sort tokens or filters. This first page of posts have no tags, so it should
  be pretty fast.

    **`GET http://localhost:8080/api/posts/?query=&limit=42`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       475 |
    | oxibooru          |        93 |
    
    Oxibooru is over 5x faster here, but I admit this is a bit of an 
    unrealistic comparison. This query doesn't perform any field selection, so
    by default all post fields will be retrieved. A decent amount of the 
    codebase is dedicated to performing efficient queries for batch retrieval
    of resource field data, so it's no surprise that it outperforms Szurubooru
    here.

- Here's a more realistic case: the query the client actually performs when
  viewing the first page of posts with no sort tokens or filters.

    **`GET http://localhost:8080/api/posts/?query=&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    
    This is the same query as before, except now we've limited the fields that 
    we're requesting. If you're wondering, `%2C` is just a `,` that has been 
    [percent encoded](https://en.wikipedia.org/wiki/Percent-encoding). I should 
    mention that this first page of posts has no tags, comments, scores, or 
    favorites, so all of these fields are fast to compute.
    
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |        55 |
    | oxibooru          |        88 |

    Now Oxibooru is about 50% _slower_ than Szurubooru. I've mostly
    spent time optimizing the more complex queries, as simple queries like 
    this are already fast. I know this query could be made at least 2x faster
    with some conditional logic, but I haven't gotten around to adding it.

- Let's now add a negative filter for unsafe posts.

    **`GET http://localhost:8080/api/posts/?query=-rating%3Aunsafe&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       101 |
    | oxibooru          |       103 |

    Oxibooru is roughly the same speed as Szurubooru here.
    
- What about with a huge offset? This query is fairly challenging, not only
  because of the large offset, but also because we have to retrieve posts
  with tags. This can be expensive because we need to retrieve statistics
  about those tags, like usage counts.

    **`GET http://localhost:8080/api/posts/?query=-rating%3Aunsafe&offset=33726&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      2490 |
    | oxibooru          |       157 |
    
    Here Oxibooru is over 15x faster than Szurubooru! One big thing that helps 
    Oxibooru go fast here is that it keeps track of usage counts for each tag 
    with triggers rather than counting them every time. This does come at a 
    cost (slower updates), but I think this  trade-off is worth it for 
    read-heavy applications like this.
    
- One very common use case is to search for posts with a particular tag. Let's
  search for all posts with the tag `tagme`.

    **`GET http://localhost:8080/api/posts/?query=tagme&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       158 |
    | oxibooru          |       161 |
    
    Szurubooru and Oxibooru perform almost identically here.
    
- Onto a more challenging query: sorting by tag count.

    **`GET http://localhost:8080/api/posts/?query=sort%3Atag-count&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      5172 |
    | oxibooru          |       133 |
    
    Oxibooru clocks in almost 40x faster! Those cached tag usage counts
    massively benefit Oxibooru here. Szurubooru counts tag usages every 
    time, so it scales poorly as the number of post tags increases.

## Listing Tags
- Just like posts, we'll start by benchmarking listing the first page of tags:

    **`GET http://localhost:8080/api/tags/?limit=50&fields=names%2Csuggestions%2Cimplications%2CcreationTime%2Cusages%2Ccategory`**
    
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       885 |
    | oxibooru          |        54 |
    
    I expected the performance from szurubooru to be reasonable here, but it
    turned out to be slower than the _autocomplete_ query. My best guess for
    why this is so slow in Szurubooru is the counting of tag usages. Though
    even before Oxibooru cached tag usages it was still 8x faster. It's
    possible that SQLAlchemy is partly to blame by generating suboptimal 
    queries, as this request was fairly tricky to implement by hand.
    
- Now let's try sorting by usage count:
    
    **`GET http://localhost:8080/api/tags/?query=sort%3Ausages&limit=50&fields=names%2Csuggestions%2Cimplications%2CcreationTime%2Cusages%2Ccategory`**

    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      6033 |
    | oxibooru          |        62 |
    
    Oxibooru just crushes Szurubooru here. It's almost 100x faster. Because
    Oxibooru keeps track of tag usages, sorting by tag count is just as
    fast as sorting by creation time.

- Finally, we'll try the query used when performing autocomplete when the user
  types the word `e` in the search bar:
    
    **`GET http://localhost:8080/api/tags/?query=e*%20sort%3Ausages&limit=15&fields=names%2Ccategory%2Cusages`**

    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       705 |
    | oxibooru          |        61 |
    
    This is an area where a speedup is very noticeable. You get much faster
    autocomplete feedback when using the search bar in Oxibooru.
    
## Reverse Image Search
- First, let's try to batch upload 50 small images each around 40-300kb. This
  way, we're mostly timing signature search and comparison rather than signature
  creation, which can be very expensive for large images. Additionaly, these 
  images don't match on any images in the database. Otherwise the time spent 
  retrieving field data about similar posts could pollute the results.
    
    **`POST http://localhost:8080/api/posts/reverse-search`**
    | Database          | Total Reverse Search Time (s) |
    | ----------------- | ----------------------------- |
    | szurubooru        |                         33.86 |
    | oxibooru          |                          6.54 |
    
    After a bunch of optimizations, Oxibooru is now 5x faster here! However,
    even this doesn't show the full story.

    Despite performing better, Oxibooru is actually doing a lot more work than 
    Szurubooru. The reverse image search method uses a two-tier approach. We have 
    a fine-grained filter that computes the "distance" between two post signatures
    and a coarse-grained filter that looks up likely candidate signatures with a 
    series of signature "words". Two signatures are considered likely to be similar
    if they have any words in common. This is faster than computing the signature 
    distance and can easily be done within Postgres, so we run the coarse-grained 
    filter first so that we don't have to run every signature through the 
    fine-grained filter. However, the coarse-grained filter is _very_ coarse. It 
    generally passes for 5-20% of all images in the database, meaning there will 
    be 8k-25k candidate signatures for a single reverse search. Szurubooru chooses 
    to limit this to the 100 candidates with the most words in common. The problem 
    with this is that as databases get larger, the likelihood of an actual match 
    falling outside of the top 100 candidate signatures increases. In Oxibooru, I 
    made the choice to evaulate _all of the candidate signatures_ against the 
    fine-grained filter.
    
- Now let's try a large image so that the signature and checksum creation time 
  dominates. Here's the results for a large 92MB image:

    | Database          | Post Creation Time (s) |
    | ----------------- | ---------------------- |
    | szurubooru        |                   8.68 |
    | oxibooru          |                   3.88 |
    
    These times are for upload, reverse image search, and post creation. The 
    reason Oxibooru is about 2x faster here is mostly because it caches the 
    checksum and image signature calculated in the reverse search so that they 
    can be reused in the post creation query. Without this caching, Oxibooru would 
    only perform slightly better than Szurubooru.