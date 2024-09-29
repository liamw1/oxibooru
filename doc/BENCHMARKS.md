# Benchmarks
These benchmarks are meant to give a rough idea of the relative speed 
differences between szurubooru and oxibooru for various operations. My 
mythodology here is very crude (often just taking a single measurement) and 
there's a lot of external factors that could affect the results, so take them 
with a grain of salt. I might work on improving these later.

These benchmarks were performed on the same database consisting of about 125k 
images and 25k tags. Everything is containerized with Docker and all requests 
are made anonymously, so time to authenticate isn't included.

1. [Startup](#startup)
2. [Listing Posts](#listing-posts)
3. [Listing Tags](#listing-tags)
4. [Reverse Image Search](#reverse-image-search)

## Startup
- Here I measure the time it takes for the first "info" request, which gathers
  statistics about the size of the database. This is the cached, so subsequent
  requests are much faster.

    **`GET http://localhost:8080/api/info`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      2287 |
    | oxibooru          |      2699 |

    Szurubooru seems slightly faster here. This was a pretty surprising result 
    to me, because in practice I find that the szurubooru homepage is quite slow
    to load from a cold start (10 seconds or more). Looking at the client logs,
    the client actually makes two info requests when viewing the homepage. It
    seems like two concurrent info requests are significantly slower than if 
    they were run sequentially. However, this is true for both oxi and szuru, so
    I'm still not sure why szurubooru is so much slower to load up in practice.
    
## Listing Posts
- Let's start with the simplest case: viewing the first page of posts with no
  sort tokens or filters.

    **`GET http://localhost:8080/api/posts/?query=&limit=42`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       435 |
    | oxibooru          |        61 |
    
    Oxibooru is quite a bit faster here, but I admit this is a bit of an 
    unrealistic comparison. This query doesn't perform any field selection, so
    by default all post fields will be retrieved. A decent amount of the oxi
    codebase is dedicated to performing efficient queries for batch retrieval
    of resource field data, so it's no surprise that it outperforms szuru here.

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
    | szurubooru        |        53 |
    | oxibooru          |        61 |

    So now szurubooru wins out. Why did I even waste my time with this rewrite 
    again?

- Let's now add a negative filter for unsafe posts.

    **`GET http://localhost:8080/api/posts/?query=-rating%3Aunsafe&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       178 |
    | oxibooru          |        72 |
    
    Now oxibooru is about 2x faster. Neat!
    
- What about with a huge offset?

    **`GET http://localhost:8080/api/posts/?query=-rating%3Aunsafe&offset=33726&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      6231 |
    | oxibooru          |      1162 |
    
    As expected, large offsets inflate the runtime quite a bit. But it's a bit
    surprising that the difference between oxi and szuru is so large here.
    
- One very common use case is to search for posts with a particular tag. Let's
  search for all posts with the tag `tagme`.

    **`GET http://localhost:8080/api/posts/?query=tagme&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |       594 |
    | oxibooru          |       133 |
    
    I'm not sure why szurubooru performs so poorly here, as both tag names
    and post tags are indexed. Perhaps SQLAlchemy is generating inefficient SQL?
    
- Onto a more challenging query: sorting by tag count.

    **`GET http://localhost:8080/api/posts/?query=sort%3Atag-count&limit=42&fields=id%2CthumbnailUrl%2Ctype%2Csafety%2Cscore%2CfavoriteCount%2CcommentCount%2Ctags%2Cversion`**
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      8298 |
    | oxibooru          |      1623 |
    
    Oxibooru clocks in around 5x faster. In general it tends to handle complex 
    queries much better.

## Listing Tags
- Just like posts, we'll start by benchmarking listing the first page of tags:

    **`GET http://localhost:8080/api/tags/?limit=50&fields=names%2Csuggestions%2Cimplications%2CcreationTime%2Cusages%2Ccategory`**
    
    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      2223 |
    | oxibooru          |        94 |
    
    I expected the performance from szurubooru to be reasonable here, but it
    turned out to be slower than the _autocomplete_ query. My best guess as to
    what's going on is that SQLAlchemy generates bad queries for collecting the
    tag field data like names, suggestions, and implications. It was pretty
    tricky to create efficient queries for these by hand, so I wouldn't be 
    surprised if ORMs struggle a bit here. Still, a 20x difference is suprising
    here.
    
- Now let's try sorting by usage count:
    
    **`GET http://localhost:8080/api/tags/?query=sort%3Ausages&limit=50&fields=names%2Csuggestions%2Cimplications%2CcreationTime%2Cusages%2Ccategory`**

    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      6517 |
    | oxibooru          |       772 |
    
    Again, oxibooru greatly outperforms szurubooru on more challenging queries.

- Finally, we'll try the query used when performing autocomplete when the user
  types the word `e` in the search bar:
    
    **`GET http://localhost:8080/api/tags/?query=e*%20sort%3Ausages&limit=15&fields=names%2Ccategory%2Cusages`**

    | Database          | Time (ms) |
    | ----------------- | --------- |
    | szurubooru        |      1207 |
    | oxibooru          |       435 |
    
    Not too dramatic of a difference this time, but large enough to be
    noticeable.
    
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

    First of all, oxibooru is actually doing a lot more
    work than szurubooru. The reverse image search method uses a two-tier
    approach. We have a fine-grained filter that computes the "distance"
    between two post signatures and a coarse-grained filter that looks up likely
    candidate signatures with a series of signature "words" (basically indexes).
    Two signatures are considered likely to be similar if they have any words
    in common. This is faster than computing the signature distance and can
    easily be done within Postgres, so we run the coarse-grained filter first so
    that we don't have to run every signature through the fine-grained filter.
    However, the coarse-grained filter is _very_ coarse. It generally passes for 
    5-20% of all images in the database, meaning there will be 8k-25k candidate
    signatures for a single reverse search. Szurubooru chooses to limit this to 
    the 100 candidates with the most words in common. The problem with this is
    that as databases get larger, the likelihood of an actual match falling
    outside of the top 100 candidate signatures increases. In oxibooru, I made
    the choice to evaulate _all of the candidate signatures_ against the
    fine-grained filter.
    
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
    starts out 2x _slower_ than szurubooru. But after the first 5 searches the 
    queries suddenly speed up dramatically (3x faster than szurubooru on 
    average) and stay that way until the end of the batch. I don't know what 
    kind of black magic is going on in Postgres, but if I can figure out how to 
    harness it I could make the first few reverse searches fast too.
    
- Now let's try a large image so that the signature and checksum creation time 
  dominates. Here's the results from an upload of a 92MB image of the Mona Lisa 
  I took from wikipedia:

    | Database          | Post Creation Time (s) |
    | ----------------- | ---------------------- |
    | szurubooru        |                   8.68 |
    | oxibooru          |                   4.81 |
    
    These times are for upload, reverse image search, and post creation.
    The reason oxibooru is about 2x faster here is because it caches the
    checksum and image signature calculated in the reverse search so that
    they can be reused in the post creation query. Without this caching,
    oxibooru would perform about as well as szurubooru. There's probably
    still some room for improvement here. I may try playing around with
    vectorizing and/or multithreading the post signature creation at some
    point.