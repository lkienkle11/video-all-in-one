# Clean machine tester checklist

Handoff file of this phase: .context/<date>-phase-<name>-handoff.md

Use a clean machine or virtual machine for the operating system you are checking. Record the app build and machine details in the handoff file. For each item, mark Pass, Fail or Not run and add a note when useful. Phase 2 is not yet available; skip those checks until it is released.

## macOS Apple silicon

| Phase | What to do | What you should see | Result and note |
|---|---|---|---|
| 1 | Install and open the development build. Follow the README steps for the macOS first-run warning. | The development app opens after the warning is cleared. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Check the app icon, then try to make the window smaller than 960 x 600. | The app icon is shown and the window stays at least 960 x 600. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | With no FFmpeg installed on the machine, add a short video and export a Fast cut and a Precise cut. | The app uses its bundled FFmpeg and both exports finish. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Put another FFmpeg version on `PATH`, then add a video and export. | The app still works and uses its bundled FFmpeg. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Remove the bundled `ffmpeg` from the app and open it. | The app says “installation is incomplete”. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Install the development and production builds side by side. Check their names and data, then uninstall the development build. | Names and data stay separate; uninstall leaves nothing in the development app's install folder. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | From the previous released production version, update in the app to the new production build. | The new build opens with the existing data and settings intact. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Start an export, open the update dialog, choose “Update after they finish”, and let the export finish. Repeat with another export and choose “Update now and stop them”. | The dialog reports the number of queued or running exports; the first update waits for the export, and the second stops it and updates. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Open data copied from a newer version with an older app version. | A window shows a plain sentence explaining that the data folder could not be opened and shows the data folder. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Leave little free disk space, then update. | The update fails clearly and the old version still works. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Put the app in a folder that is not writable, then update. | The app shows the downloaded file for a manual install. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Add a video; check the file filter and folder dialog. | The file filter and folder dialog work. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Use each “set from preview” button. | Each button sets the value from the preview. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Choose a save folder that cannot be written to. Check immediately, focus the window again, press Export, and queue a job. | Each check point reports that the folder cannot be written to; the export error appears as a toast. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Choose a writable save folder, delete it, then export. | The folder is recreated and an Info toast appears. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Run a batch with several failures. | One toast appears; the failure details are in the queue. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Check a queued video. | The queue shows the video name and “Copy details”. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2b | Read every screen and report any wording that is unclear. | Unclear wording is recorded in the handoff. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 3 | Play a video in the preview and listen. | Preview sound plays clearly. | [ ] Pass [ ] Fail [ ] Not run — Note: |

## macOS Intel

| Phase | What to do | What you should see | Result and note |
|---|---|---|---|
| 1 | Install and open the development build. Follow the README steps for the macOS first-run warning. | The development app opens after the warning is cleared. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Check the app icon, then try to make the window smaller than 960 x 600. | The app icon is shown and the window stays at least 960 x 600. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | With no FFmpeg installed on the machine, add a short video and export a Fast cut and a Precise cut. | The app uses its bundled FFmpeg and both exports finish. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Put another FFmpeg version on `PATH`, then add a video and export. | The app still works and uses its bundled FFmpeg. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Remove the bundled `ffmpeg` from the app and open it. | The app says “installation is incomplete”. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Install the development and production builds side by side. Check their names and data, then uninstall the development build. | Names and data stay separate; uninstall leaves nothing in the development app's install folder. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | From the previous released production version, update in the app to the new production build. | The new build opens with the existing data and settings intact. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Start an export, open the update dialog, choose “Update after they finish”, and let the export finish. Repeat with another export and choose “Update now and stop them”. | The dialog reports the number of queued or running exports; the first update waits for the export, and the second stops it and updates. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Open data copied from a newer version with an older app version. | A window shows a plain sentence explaining that the data folder could not be opened and shows the data folder. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Leave little free disk space, then update. | The update fails clearly and the old version still works. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Put the app in a folder that is not writable, then update. | The app shows the downloaded file for a manual install. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Add a video; check the file filter and folder dialog. | The file filter and folder dialog work. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Use each “set from preview” button. | Each button sets the value from the preview. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Choose a save folder that cannot be written to. Check immediately, focus the window again, press Export, and queue a job. | Each check point reports that the folder cannot be written to; the export error appears as a toast. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Choose a writable save folder, delete it, then export. | The folder is recreated and an Info toast appears. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Run a batch with several failures. | One toast appears; the failure details are in the queue. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Check a queued video. | The queue shows the video name and “Copy details”. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2b | Read every screen and report any wording that is unclear. | Unclear wording is recorded in the handoff. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 3 | Play a video in the preview and listen. | Preview sound plays clearly. | [ ] Pass [ ] Fail [ ] Not run — Note: |

## Windows 10 or later

| Phase | What to do | What you should see | Result and note |
|---|---|---|---|
| 1 | Install and open the development build. Follow the README steps for the Windows first-run warning. | The development app opens after the warning is cleared, with no console window. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Check the app icon, then try to make the window smaller than 960 x 600. | The app icon is shown and the window stays at least 960 x 600. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | With no FFmpeg installed on the machine, add a short video and export a Fast cut and a Precise cut. | The app uses its bundled FFmpeg and both exports finish. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Put another FFmpeg version on `PATH`, then add a video and export. | The app still works and uses its bundled FFmpeg. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Remove the bundled `ffmpeg` from the install folder and open the app. | The app says “installation is incomplete”. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Export a Precise cut. | Precise cut works with `h264_mf` or OpenH264, as built. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Install the development and production builds side by side. Check their names and data, then uninstall the development build. | Names and data stay separate; uninstall leaves nothing in the development app's install folder. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | From the previous released production version, update in the app to the new production build. | The new build opens with the existing data and settings intact. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Start an export, open the update dialog, choose “Update after they finish”, and let the export finish. Repeat with another export and choose “Update now and stop them”. | The dialog reports the number of queued or running exports; the first update waits for the export, and the second stops it and updates. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Open data copied from a newer version with an older app version. | A window shows a plain sentence explaining that the data folder could not be opened and shows the data folder. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Leave little free disk space, then update. | The update fails clearly and the old version still works. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Update while the app is open. On a build that renames a DLL, check `<install folder>\bin` after the update. | The update completes and no old DLL remains in `bin`. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Force a failure in the middle of an update, for example by locking one file with another program. | The old files are restored and the app shows “The last update did not finish”. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Add a video; check the file filter and folder dialog. | The file filter and folder dialog work. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Use each “set from preview” button. | Each button sets the value from the preview. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Choose a save folder that cannot be written to. Check immediately, focus the window again, press Export, and queue a job. | Each check point reports that the folder cannot be written to; the export error appears as a toast. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Choose a writable save folder, delete it, then export. | The folder is recreated and an Info toast appears. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Choose a folder protected by Controlled folder access and export there. | The app reports the write failure. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Run a batch with several failures. | One toast appears; the failure details are in the queue. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Check a queued video. | The queue shows the video name and “Copy details”. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2b | Read every screen and report any wording that is unclear. | Unclear wording is recorded in the handoff. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 3 | Play a video in the preview and listen. | Preview sound plays clearly. | [ ] Pass [ ] Fail [ ] Not run — Note: |

## Ubuntu 22.04

| Phase | What to do | What you should see | Result and note |
|---|---|---|---|
| 1 | Install and open the development build. | The development app opens. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Check the app icon, then try to make the window smaller than 960 x 600. | The app icon is shown and the window stays at least 960 x 600. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | With no FFmpeg installed on the machine, add a short video and export a Fast cut and a Precise cut. | The app uses its bundled FFmpeg and both exports finish. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Put another FFmpeg version on `PATH`, then add a video and export. | The app still works and uses its bundled FFmpeg. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Remove the bundled `ffmpeg` from the install folder and open the app. | The app says “installation is incomplete”. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Install the development and production builds side by side. Check their names and data, then uninstall the development build. | Names and data stay separate; uninstall leaves nothing in the development app's install folder. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | From the previous released production version, update in the app to the new production build. | The new build opens with the existing data and settings intact. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Start an export, open the update dialog, choose “Update after they finish”, and let the export finish. Repeat with another export and choose “Update now and stop them”. | The dialog reports the number of queued or running exports; the first update waits for the export, and the second stops it and updates. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Open data copied from a newer version with an older app version. | A window shows a plain sentence explaining that the data folder could not be opened and shows the data folder. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Leave little free disk space, then update. | The update fails clearly and the old version still works. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 1 | Update when the new package adds a dependency that is not installed; remove that dependency first. | The update installs the missing dependency and completes. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Add a video; check the file filter and folder dialog. | The file filter and folder dialog work. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Use each “set from preview” button. | Each button sets the value from the preview. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Choose a save folder that cannot be written to. Check immediately, focus the window again, press Export, and queue a job. | Each check point reports that the folder cannot be written to; the export error appears as a toast. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Choose a writable save folder, delete it, then export. | The folder is recreated and an Info toast appears. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Run a batch with several failures. | One toast appears; the failure details are in the queue. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2 (not yet available) | Check a queued video. | The queue shows the video name and “Copy details”. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 2b | Read every screen and report any wording that is unclear. | Unclear wording is recorded in the handoff. | [ ] Pass [ ] Fail [ ] Not run — Note: |
| 3 | Play a video in the preview and listen. | Preview sound plays clearly. | [ ] Pass [ ] Fail [ ] Not run — Note: |
