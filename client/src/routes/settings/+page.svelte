<script>
  let settings = [
    {
      name: 'keyboard-shortcuts',
      text: 'Enable keyboard shortcuts',
      hint: null,
      type: 'checkbox',
      setting: {
        checked: true,
      },
    },
    {
      name: 'posts-per-page',
      text: 'Number of posts per page',
      hint: null,
      type: 'range',
      setting: {
        value: 42,
        min: 10,
        max: 100,
      },
    },
    {
      name: 'dark-theme',
      text: 'Use dark theme',
      hint: 'Changing this setting will require you to refresh the page for it to apply.',
      type: 'checkbox',
      setting: {
        checked: true,
      },
    },
    {
      name: 'upscale-small-posts',
      text: 'Upscale small posts',
      hint: null,
      type: 'checkbox',
      setting: {
        checked: true,
      },
    },
    {
      name: 'endless-scroll',
      text: 'Enable endless scroll',
      hint:
        'Rather than using a paged navigation, smoothly scrolls through the content.',
      type: 'checkbox',
      setting: {
        checked: true,
      },
    },
    {
      name: 'post-flow',
      text: 'Use post flow',
      hint: 'Use a content-aware flow for thumbnails on the post search page.',
      type: 'checkbox',
      setting: {
        checked: true,
      },
    },
    {
      name: 'transparency-grid',
      text: 'Enable transparency grid',
      hint:
        'Renders a checkered pattern behind posts with transparent background.',
      type: 'checkbox',
      setting: {
        checked: true,
      },
    },
    {
      name: 'tag-suggestions',
      text: 'Show tag suggestions',
      hint: 'Shows a popup with suggested tags in edit forms.',
      type: 'checkbox',
      setting: {
        checked: true,
      },
    },
    {
      name: 'autoplay-videos',
      text: 'Automatically play video posts',
      hint: null,
      type: 'checkbox',
      setting: {
        checked: false,
      },
    },
    {
      name: 'underscores-as-spaces',
      text: 'Display underscores as spaces',
      hint:
        "Display all underscores as if they were spaces. This is only a visual change, which means that you'll still have to use underscores when searching or editing tags.",
      type: 'checkbox',
      setting: {
        checked: false,
      },
    },
  ]
</script>

<!-- FIXME: hints needs to cap at the half of the card or use tooltip with help icon-->
<div class="hero min-h-screen">
  <div class="card w-96 bg-base-300 shadow-xl m-10">
    <form>
      <div class="card-body">
        <h2 class="card-title">Browsing settings</h2>
        <p class="text-xs">
          These settings are saved to the browser's local storage and are not
          coupled to the user account, so they don't apply to other devices or
          browsers alike.
        </p>
        <div class="flex flex-col">
          <div class="form-control w-auto">
            <ul>
              {#each settings as { name, text, hint, type, setting }}
                <li class="mb-5">
                  <label class="cursor-pointer label">
                    {#if type === 'checkbox'}
                      <input
                        {type}
                        {name}
                        class="toggle toggle-primary mr-5"
                        checked={setting.checked} />
                    {:else if type === 'range'}
                      <!-- TODO: get value to see what actually has been set -->
                      <input
                        type="range"
                        bind:value={setting.value}
                        {name}
                        min={setting.min}
                        max={setting.max}
                        class="range range-primary mr-5"
                        checked={setting.checked} />
                      <span>{setting.value}</span>
                    {/if}
                    <span class="label-text">{text}</span>
                  </label>
                  {#if hint}
                    <p class="text-xs text-right">{hint}</p>
                  {/if}
                </li>
              {/each}
            </ul>
          </div>
        </div>
        <div class="card-actions">
          <input
            type="submit"
            value="Save"
            class="btn btn-primary btn-sm min-w-full" />
        </div>
      </div>
    </form>
  </div>
</div>
