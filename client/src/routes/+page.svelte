<script>
  let instanceName = 'imageArchive'
  let featuredPost = {
    id: 285802,
    poster: {
      user: 'phil',
      pfp: '/phil_placeholder.png',
    },
    time: '2022-10-29T01:20:03.468789Z',
    file: '/featured_placeholder.jpg',
  }
  // let featuredPost = null;
</script>

<div
  class="hero flex-auto"
  style="{featuredPost ? `background-image: url(${featuredPost.file});` : ''}">
  <div class="hero-overlay bg-opacity-60" />
  <div class="hero-content flex-col">
    <!-- title with search -->
    <header>
      <h1 class="text-5xl m-10">{instanceName}</h1>
    </header>
    <form>
      <input
        type="text"
        placeholder="Tag Search"
        class="input input-primary input-sm" />
      <input type="submit" value="Search" class="btn btn-primary btn-sm" />
    </form>
    <a href="posts" class="btn btn-secondary btn-wide btn-xs">
      browse all posts
    </a>
    <!-- featured post text-->
    {#if featuredPost}
      <aside class="mt-5">
        Featured post:
        <!-- TODO: hover, hide all and preview bg-->
        <a href="post/{featuredPost.id}" class="link link-info">
          @{featuredPost.id.toString()}
        </a>
        <!-- FIXME: comma doesnt join the featured post ID -->
        , posted
        <time datetime={featuredPost.time} title={featuredPost.time}>
          a month ago
        </time>
        by
        <span>
          <!-- FIXME: better thumbnail implementation -->
          <a href="post/{featuredPost.poster.user}" class="link link-info">
            <span class="avatar">
              <div class="w-5">
                <img src={featuredPost.poster.pfp} alt="thumbnail" />
              </div>
            </span>
            {featuredPost.poster.user}
          </a>
        </span>
      </aside>
    {/if}
  </div>
</div>
