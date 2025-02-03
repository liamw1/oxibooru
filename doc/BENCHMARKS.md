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
    | oxibooru          |        99 |
    
    Oxibooru is over 4x faster here, but I admit this is a bit of an 
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
    | oxibooru          |        85 |

    Now Oxibooru is about 50% _slower_ than Szurubooru. I've mostly
    spent time optimizing the more complex queries, as simple queries like 
    this are already fast. I know this query could be made at least 2x faster
    with some conditional logic, but I haven't gotten around to adding it.

- Let's now add a negative filter for unsafe posts.

    **`GET http://localhost:8080/api/posts/?query=-rating%3Aunsafe&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       101 |
    | oxibooru          |       125 |

    Oxibooru is only 25% slower here.
    
- What about with a huge offset? This query is fairly challenging, not only
  because of the large offset, but also because we have to retrieve posts
  with tags. This can be expensive because we need to retrieve statistics
  about those tags, like usage counts.

    **`GET http://localhost:8080/api/posts/?query=-rating%3Aunsafe&offset=33726&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      2490 |
    | oxibooru          |       193 |
    
    Here Oxibooru is over 10x faster than Szurubooru! One big thing that helps 
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
    | oxibooru          |       154 |
    
    Szurubooru and Oxibooru perform almost identically here.
    
- Onto a more challenging query: sorting by tag count.

    **`GET http://localhost:8080/api/posts/?query=sort%3Atag-count&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      5172 |
    | oxibooru          |       171 |
    
    Oxibooru clocks in around 30x faster! Those cached tag usage counts
    massively benefit Oxibooru here. Szurubooru counts tag usages every 
    time, so it scales poorly as the number of post tags increases.

## Listing Tags
- Just like posts, we'll start by benchmarking listing the first page of tags:

    **`GET http://localhost:8080/api/tags/?limit=50&fields=names%2Csuggestions%2Cimplications%2CcreationTime%2Cusages%2Ccategory`**
    
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       885 |
    | oxibooru          |        73 |
    
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
    | oxibooru          |        88 |
    
    Oxibooru just crushes Szurubooru here. It's over 60x faster. Because
    Oxibooru keeps track of tag usages, sorting by tag count is just as
    fast as sorting by creation time.

- Finally, we'll try the query used when performing autocomplete when the user
  types the word `e` in the search bar:
    
    **`GET http://localhost:8080/api/tags/?query=e*%20sort%3Ausages&limit=15&fields=names%2Ccategory%2Cusages`**

    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       705 |
    | oxibooru          |        56 |
    
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
    | oxibooru          |                         16.27 |
    
    Oxibooru is more than 2x faster here, so it looks like the time I spent on
    optimizing image search paid off! However, the total runtime doesn't show
    the full story here.

    First of all, Oxibooru is actually doing a lot more work than Szurubooru. 
    The reverse image search method uses a two-tier approach. We have a 
    fine-grained filter that computes the "distance" between two post 
    signatures and a coarse-grained filter that looks up likely candidate 
    signatures with a series of signature "words". Two signatures are considered
    likely to be similar if they have any words in common. This is faster than 
    computing the signature distance and can easily be done within Postgres, so 
    we run the coarse-grained filter first so that we don't have to run every 
    signature through the fine-grained filter. However, the coarse-grained 
    filter is _very_ coarse. It generally passes for 5-20% of all images in the 
    database, meaning there will be 8k-25k candidate signatures for a single 
    reverse search. Szurubooru chooses to limit this to the 100 candidates with 
    the most words in common. The problem with this is that as databases get 
    larger, the likelihood of an actual match falling outside of the top 100 
    candidate signatures increases. In oxibooru, I made the choice to evaulate 
    _all of the candidate signatures_ against the fine-grained filter.
    
    Secondly, something interesting is going on when we look at the times for
    the individual reverse search queries. Here are the first 10:
    
    | Image Number | Szurubooru Time (ms) | Oxibooru Time (ms) |
    | ------------ | -------------------- | ------------------ |
    |            1 |                  683 |               1238 |
    |            2 |                  669 |               1217 |
    |            3 |                  737 |               1227 |
    |            4 |                  673 |               1136 |
    |            5 |                  662 |               1237 |
    |            6 |                  697 |                 82 |
    |            7 |                  664 |                236 |
    |            8 |                  671 |                241 |
    |            9 |                  740 |                311 |
    |           10 |                  666 |                199 |
    
    The reverse search times for szurubooru are fairly consistent, hovering 
    around 670ms each. On the other hand, the oxibooru reverse search actually 
    starts out 2x _slower_ than szurubooru. But after the first 5 searches, the 
    queries suddenly speed up dramatically (3x faster than szurubooru on 
    average) and stay that way until the end of the batch. 
    
    My best guess as to what's going on here is that each time a query is 
    executed, some of the signature rows are cached. Because each request is 
    expected to find around 20% of the signatures to be likely matches, it takes
    ~5 requests for most of the rows to be cached. Once that happends, things 
    can progress at reasonable speed. Perhaps there's some way of pre-loading 
    the signature table into cache? In any case, if I can harness whatever black
    magic is going on in Postgres, maybe I could make the first few reverse 
    searches fast too.
    
- Now let's try a large image so that the signature and checksum creation time 
  dominates. Here's the results from an upload of a 92MB image of the Mona Lisa 
  I took from [wikipedia](https://en.wikipedia.org/wiki/Mona_Lisa#/media/File:Mona_Lisa,_by_Leonardo_da_Vinci,_from_C2RMF_retouched.jpg):

    | Database          | Post Creation Time (s) |
    | ----------------- | ---------------------- |
    | szurubooru        |                   8.68 |
    | oxibooru          |                   4.51 |
    
    These times are for upload, reverse image search, and post creation. The 
    reason oxibooru is about 2x faster here is because it caches the checksum 
    and image signature calculated in the reverse search so that they can be
    reused in the post creation query. Without this caching, oxibooru would 
    perform about as well as szurubooru. There's probably still some room for 
    improvement here. The runtime is a somewhat even split between the png 
    decoding, the thumbnail creation, and the image signature creation. The 
    first two are largely out of my control, but I may try playing around with 
    vectorizing and/or multithreading the post signature creation at some point.