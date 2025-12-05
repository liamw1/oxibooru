<script>
  let userinfo = {
    pfp: '/phil_placeholder.png'
  }
</script>

<div class="navbar bg-base-300 p-0 min-h-0 shadow-xl">
  <div class="flex-1">
    <ul class="menu menu-horizontal">
      <li><a href="/">Home</a></li>
      <li><a href="/posts">Posts</a></li>
      <li><a href="/upload">Upload</a></li>
      <li><a href="/comments">Comments</a></li>
      <li><a href="/tags">Tags</a></li>
      <li><a href="/pools">Pools</a></li>
      <li><a href="/users">Users</a></li>
    </ul>
  </div>
  <div class="flex-none">
    <ul class="menu menu-horizontal">
      <li>
        <a href="user/USERNAME">
          <span class="avatar">
            <div class="w-5">
              <img src={userinfo.pfp} alt="thumbnail" />
            </div>
          </span>
        Account
        </a>
      </li>
      <li><a href="/register">Register</a></li>
      <li><a href="/logout">Logout</a></li>
      <li><a href="/help">Help</a></li>
      <li><a href="/settings">⚙️</a></li>
    </ul>
  </div>
</div>