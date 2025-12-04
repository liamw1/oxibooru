// Edit on Github https://github.com/docsifyjs/docsify/blob/167596be51d8918efa2238a0e12eab14445e4022/docs/index.html#L193C16-L193C16
// footer https://github.com/boopathikumar018/docsify-darklight-theme/blob/master/docs/index.html        
function siteWrapperPlugin(hook, vm) {
  const footer = [
    '<footer style="text-align: center; padding-top: 40px;">',
    'Oxibooru &copy; 2025',
    "<br/>",
    'Made with <span style="color: lightcoral">❤</span> by <a href="https://github.com/liamw1/oxibooru" target="_blank">@Oxibooru</a> and <a href="https://github.com/liamw1/oxibooru/graphs/contributors" target="_blank">Helpers</a>',
    "</footer>",
  ].join("");

  hook.doneEach(() => {
    const html = document.querySelector('main section article');

    // footer
    const parsedAfter = new DOMParser().parseFromString([footer].join(''), "text/html");
    html.append(parsedAfter.body);

    // if 3rd-Party articles are being used, fixup the link for the Edit button
    if (/githubusercontent\.com/.test(vm.route.file)) {
      url = vm.route.file
        .replace('raw.githubusercontent.com', 'github.com')
        .replace(/\/master/, '/blob/master');
    } else if (/jsdelivr\.net/.test(vm.route.file)) {
      url = vm.route.file
        .replace('cdn.jsdelivr.net/gh', 'github.com')
        .replace('@master', '/blob/master');
    } else {
      url =
        'https://github.com/liamw1/oxibooru/blob/main/' +
        vm.route.file;
    }

    const title = html.querySelector('h1');
    const editButtonHtml = `<a href="${url}" target="_blank"><svg class="edit-page" xmlns="http://www.w3.org/2000/svg" height="22" width="22" viewBox="0 0 512 512"><!--!Font Awesome Free 6.6.0 by @fontawesome - https://fontawesome.com License - https://fontawesome.com/license/free Copyright 2024 Fonticons, Inc.--><path d="M410.3 231l11.3-11.3-33.9-33.9-62.1-62.1L291.7 89.8l-11.3 11.3-22.6 22.6L58.6 322.9c-10.4 10.4-18 23.3-22.2 37.4L1 480.7c-2.5 8.4-.2 17.5 6.1 23.7s15.3 8.5 23.7 6.1l120.3-35.4c14.1-4.2 27-11.8 37.4-22.2L387.7 253.7 410.3 231zM160 399.4l-9.1 22.7c-4 3.1-8.5 5.4-13.3 6.9L59.4 452l23-78.1c1.4-4.9 3.8-9.4 6.9-13.3l22.7-9.1 0 32c0 8.8 7.2 16 16 16l32 0zM362.7 18.7L348.3 33.2 325.7 55.8 314.3 67.1l33.9 33.9 62.1 62.1 33.9 33.9 11.3-11.3 22.6-22.6 14.5-14.5c25-25 25-65.5 0-90.5L453.3 18.7c-25-25-65.5-25-90.5 0zm-47.4 168l-144 144c-6.2 6.2-16.4 6.2-22.6 0s-6.2-16.4 0-22.6l144-144c6.2-6.2 16.4-6.2 22.6 0s6.2 16.4 0 22.6z"/></svg></a>`;
    const editButton = new DOMParser().parseFromString(editButtonHtml, "text/html");
    const titleDiv = document.createElement("div");
    titleDiv.className = 'page-header';
    titleDiv.appendChild(title);
    titleDiv.appendChild(...editButton.body.childNodes);
    html.prepend(titleDiv);
  });
}
// Add plugin to docsify's plugin array
window.$docsify = window.$docsify || {};
$docsify.plugins = [siteWrapperPlugin, ...($docsify.plugins || [])];
