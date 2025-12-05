<script>
  let usage = 0;
  let postAmount = 0;
  let postSpelling = postAmount !== 1 ? 's' : '';
</script>

<footer class="footer footer-center p-10 bg-base-300 text-base-content gap-2">
  <div class="grid grid-flow-col gap-4">
    <p>{postAmount.toString()} post{postSpelling}</p>
    <p>{usage.toString()} GB</p>
    <a href="history" class="link link-info">History</a>
  </div>
  <div>
    <p>
      Build
      <a class="link" href="https://github.com/rr-/szurubooru/commits/master">docker-latest</a>
      from
      <time datetime="Thu, 09 Jun 2022 00:36:28 GMT" title="Thu, 09 Jun 2022 00:36:28 GMT"> 6 months ago </time>
    </p>
  </div>
</footer>