// SPDX-License-Identifier: AGPL-3.0-or-later
// What the player asks the server, as web's Player.tsx does.

import { graphql } from '../gql'

export const PlaybackQuery = graphql(`
  query Playback($id: Int!) {
    video(id: $id) {
      ...Playback
    }
    server {
      transcoding {
        ...TranscodingFields
      }
    }
  }
`)

export const SaveProgress = graphql(`
  mutation SaveProgress($videoId: Int!, $position: Float!, $duration: Float!) {
    saveProgress(videoId: $videoId, position: $position, duration: $duration) {
      id
    }
  }
`)

export const TakeScreenshot = graphql(`
  mutation TakeScreenshot($input: NewScreenshot!) {
    takeScreenshot(input: $input) {
      ...ClipFields
    }
  }
`)

export const ScheduleQuery = graphql(`
  query PlayerSchedule($id: Int!) {
    title(id: $id) {
      series {
        id
        monitor
        status
        next {
          ...SeriesEpisodeFields
        }
      }
    }
  }
`)

export const OverviewQuery = graphql(`
  query PlayerOverview($id: Int!, $videoId: Int!) {
    title(id: $id) {
      overview
    }
    video(id: $videoId) {
      overview
    }
  }
`)
