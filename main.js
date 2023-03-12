(()=>{function g(){}var ue=0,x=class extends m{constructor(){super();this.error=g,this.warn=g,this.info=g,this.debug=g,this._currentLevel=ue}setLevel(t){let n=t<0||t>4?0:t;this._currentLevel=n,this.error=n>=1?console.error.bind(console):g,this.warn=n>=2?console.warn.bind(console):g,this.info=n>=3?console.info.bind(console):g,this.debug=n>=4?console.debug.bind(console):g,this.trigger("onLogLevelChange",n)}getLevel(){return this._currentLevel}hasLevel(t){return t>=this._currentLevel}},de=new x,a=de;var m=class{constructor(){this._listeners={}}addEventListener(e,t){let n=this._listeners[e];Array.isArray(n)?n.push(t):this._listeners[e]=[t]}removeEventListener(e,t){if(e===void 0){this._listeners={};return}let n=this._listeners[e];if(!Array.isArray(n))return;if(t===void 0){delete this._listeners[e];return}let o=n.indexOf(t);o!==-1&&n.splice(o,1),n.length===0&&delete this._listeners[e]}trigger(e,t){let n=this._listeners[e];!Array.isArray(n)||n.slice().forEach(o=>{try{o(t)}catch(i){a.error("EventEmitter: listener error",i instanceof Error?i:null)}})}};function W(){let r="",e=-1;return function(){return e++,e>=Number.MAX_SAFE_INTEGER&&(r+="0",e=0),r+String(e)}}var ce=new TextDecoder("utf-8",{ignoreBOM:!0,fatal:!0});ce.decode();var Ee=new Uint8Array;var Ie=new Int32Array;var Pe=new Uint32Array;var Te=new Float64Array;var C=new TextEncoder("utf-8"),xe=typeof C.encodeInto=="function"?function(r,e){return C.encodeInto(r,e)}:function(r,e){let t=C.encode(r);return e.set(t),{read:r.length,written:t.length}};var We=Object.freeze({MultiVariantPlaylist:0,0:"MultiVariantPlaylist",MediaPlaylist:1,1:"MediaPlaylist"}),Ce=Object.freeze({MediaSourceAttachmentError:0,0:"MediaSourceAttachmentError",Unknown:1,1:"Unknown"}),Le=Object.freeze({AlreadyCreatedWithSameType:0,0:"AlreadyCreatedWithSameType",CantPlayType:1,1:"CantPlayType",EmptyMimeType:2,2:"EmptyMimeType",MediaSourceIsClosed:3,3:"MediaSourceIsClosed",NoMediaSourceAttached:4,4:"NoMediaSourceAttached",QuotaExceededError:5,5:"QuotaExceededError",Unknown:6,6:"Unknown"}),S=Object.freeze({Timeout:0,0:"Timeout",Status:1,1:"Status",Error:2,2:"Error",Other:3,3:"Other"}),Ae=Object.freeze({NoMediaSourceAttached:0,0:"NoMediaSourceAttached",UnknownError:1,1:"UnknownError"}),Oe=Object.freeze({NoMediaSourceAttached:0,0:"NoMediaSourceAttached",UnknownError:1,1:"UnknownError"}),Re=Object.freeze({UnknownError:0,0:"UnknownError",NoContentLoaded:1,1:"NoContentLoaded"}),Be=Object.freeze({SourceBufferNotFound:0,0:"SourceBufferNotFound",UnknownError:1,1:"UnknownError"}),je=Object.freeze({NoMediaSourceAttached:0,0:"NoMediaSourceAttached",UnknownError:1,1:"UnknownError"}),Ue=Object.freeze({NoMediaSourceAttached:0,0:"NoMediaSourceAttached",MediaSourceIsClosed:1,1:"MediaSourceIsClosed",QuotaExceededError:2,2:"QuotaExceededError",TypeNotSupportedError:3,3:"TypeNotSupportedError",EmptyMimeType:4,4:"EmptyMimeType",UnknownError:5,5:"UnknownError"}),Ne=Object.freeze({NoResource:0,0:"NoResource",NoSourceBuffer:1,1:"NoSourceBuffer",TransmuxerError:2,2:"TransmuxerError",UnknownError:3,3:"UnknownError"}),qe=Object.freeze({Init:0,0:"Init",Seeked:1,1:"Seeked",Seeking:2,2:"Seeking",Ended:3,3:"Ended",ReadyStateChanged:4,4:"ReadyStateChanged",RegularInterval:5,5:"RegularInterval",Error:6,6:"Error"}),Ve=Object.freeze({MediaPlaylistRefresh:0,0:"MediaPlaylistRefresh",RetryRequest:1,1:"RetryRequest"}),ze=Object.freeze({Error:0,0:"Error",Warn:1,1:"Warn",Info:2,2:"Info",Debug:3,3:"Debug"}),L=Object.freeze({Audio:0,0:"Audio",Video:1,1:"Video"}),w=Object.freeze({Closed:0,0:"Closed",Ended:1,1:"Ended",Open:2,2:"Open"}),_=Object.freeze({Init:0,0:"Init",Seeking:1,1:"Seeking",Seeked:2,2:"Seeked",RegularInterval:3,3:"RegularInterval",LoadedData:4,4:"LoadedData",LoadedMetadata:5,5:"LoadedMetadata",CanPlay:6,6:"CanPlay",CanPlayThrough:7,7:"CanPlayThrough",Ended:8,8:"Ended",Pause:9,9:"Pause",Play:10,10:"Play",RateChange:11,11:"RateChange",Stalled:12,12:"Stalled"});var v=class extends Error{constructor(t,n,o){super();Object.setPrototypeOf(this,v.prototype),this.name="InitializationError",this.code=t,this.wasmHttpStatus=n,this.message=o}},y=class extends Error{constructor(t,n){super();Object.setPrototypeOf(this,y.prototype),this.name="WaspSegmentRequestError",this.reason=t.reason,this.url=t.url,this.isInit=t.isInit;let o=n;if(o===void 0){let{mediaType:i}=t,s=le(i)+" segment";switch(t.reason){case S.Status:o=t.status===void 0?`${s}'s HTTP(S) request(s) responded with an invalid status`:`${s}'s HTTP(S) request(s) responded with a ${t.status} status`;break;case S.Timeout:o=`${s}'s HTTP(S) request(s) did not respond`;break;case S.Error:o=`${s}'s HTTP(S) request(s) failed due to an error.`;break;case S.Other:o=`${s}'s HTTP(S) request(s) failed for an unknown reason.`;break}}this.message=o??"An error arised while trying to perform a segment request"}},M=class extends Error{constructor(t,n){super();Object.setPrototypeOf(this,M.prototype),this.name="WaspOtherError",this.code=t,this.message=n??"Unknown error"}},b=class extends Error{constructor(t,n){super();Object.setPrototypeOf(this,b.prototype),this.name="WaspSourceBufferCreationError",this.code=t,this.message=n??"Unknown error when creating SourceBuffer"}},P=class extends Error{constructor(t,n,o){super();Object.setPrototypeOf(this,b.prototype),this.name="WaspPlaylistParsingError",this.playlistType=t,this.mediaType=n,this.message=o??"Unknown error when parsing Playlist"}};function le(r){switch(r){case L.Audio:return"An audio";case L.Video:return"A video"}throw new Error("Unknown MediaType")}function c(r,e,t){a.debug("--> sending to worker:",e.type),t===void 0?r.postMessage(e):r.postMessage(e,t)}var A=(o=>(o.Stopped="Stopped",o.Loading="Loading",o.Loaded="Loaded",o.Error="Error",o))(A||{});function h(r,e){return r instanceof Error?{message:r.message,name:r.name}:{message:e,name:void 0}}function T(r,e){r.stopPlaybackObservations?.(),r.loadingAborter?.abort(),e!==null&&c(e,{type:"stop",value:{contentId:r.contentId}})}function j(r,e){return new Promise((t,n)=>{r.readyState>=HTMLMediaElement.HAVE_ENOUGH_DATA&&t(),e.addEventListener("abort",i),r.addEventListener("canplay",o);function o(){r.removeEventListener("canplay",o),e.removeEventListener("abort",i),t()}function i(){r.removeEventListener("canplay",o),e.removeEventListener("abort",i),e.reason!==null?n(e.reason):n(new Error("The loading operation was aborted"))}})}function U(r){let{textTracks:e}=r;if(e!=null){for(let t=0;t<e.length;t++)e[t].mode="disabled";if(r.hasChildNodes()){let{childNodes:t}=r;for(let n=t.length-1;n>=0;n--)if(t[n].nodeName==="track")try{r.removeChild(t[n])}catch{}}}r.src="",r.removeAttribute("src")}var E=class{constructor(e){this._sourceBuffer=e,this._queue=[],this._pendingTask=null;let t=setInterval(()=>{this._flush()},2e3),n=this._onPendingTaskError.bind(this),o=()=>{this._flush()};e.addEventListener("error",n),e.addEventListener("updateend",o),this._dispose=[()=>{clearInterval(t),e.removeEventListener("error",n),e.removeEventListener("updateend",o)}]}push(e){return a.debug("QSB: receiving order to push data to the SourceBuffer"),this._addToQueue({type:0,value:e})}removeBuffer(e,t){return a.debug("QSB: receiving order to remove data from the SourceBuffer",e,t),this._addToQueue({type:1,value:{start:e,end:t}})}getBufferedRanges(){return this._sourceBuffer.buffered}dispose(){for(this._dispose.forEach(e=>e()),this._pendingTask!==null&&(this._pendingTask.reject(new Error("QueuedSourceBuffer Cancelled")),this._pendingTask=null);this._queue.length>0;){let e=this._queue.shift();e!==void 0&&e.reject(new Error("QueuedSourceBuffer Cancelled"))}}_onPendingTaskError(e){let t=e instanceof Error?e:new Error("An unknown error occured when doing operations on the SourceBuffer");this._pendingTask!=null&&this._pendingTask.reject(t)}_addToQueue(e){return new Promise((t,n)=>{let o=this._queue.length===0&&this._pendingTask===null,i={resolve:t,reject:n,...e};this._queue.push(i),o&&this._flush()})}_flush(){if(!this._sourceBuffer.updating){if(this._pendingTask!==null){let e=this._pendingTask,{resolve:t}=e;return this._pendingTask=null,t(),this._flush()}else{let e=this._queue.shift();if(e===void 0)return;this._pendingTask=e}try{switch(this._pendingTask.type){case 0:let e=this._pendingTask.value;if(e===void 0){this._flush();return}a.debug("QSB: pushing data"),this._sourceBuffer.appendBuffer(e);break;case 1:let{start:t,end:n}=this._pendingTask.value;a.debug("QSB: removing data from SourceBuffer",t,n),this._sourceBuffer.remove(t,n);break;default:fe(this._pendingTask)}}catch(e){this._onPendingTaskError(e)}}}};function fe(r){throw new Error("Unreachable path taken")}var _e=[["seeking",_.Seeking],["seeked",_.Seeked],["loadedmetadata",_.LoadedMetadata],["loadeddata",_.LoadedData],["canplay",_.CanPlay],["canplaythrough",_.CanPlayThrough],["ended",_.Ended],["pause",_.Pause],["play",_.Play],["ratechange",_.RateChange],["stalled",_.Stalled]];function O(r,e,t){let n=!1,o,i=_e.map(([u,f])=>{r.addEventListener(u,l);function l(){s(f)}return()=>r.removeEventListener(u,l)});return Promise.resolve().then(()=>s(_.Init)),()=>{n||(n=!0,i.forEach(u=>u()),i.length=0,o!==void 0&&(clearTimeout(o),o=void 0))};function s(u){if(n)return;o!==void 0&&(clearTimeout(o),o=void 0);let f=new Float64Array(r.buffered.length*2);for(let k=0;k<r.buffered.length;k++){let B=k*2;f[B]=r.buffered.start(k),f[B+1]=r.buffered.end(k)}let{currentTime:l,readyState:d,paused:p,seeking:R,ended:ie,duration:se}=r;t({mediaSourceId:e,reason:u,currentTime:l,readyState:d,buffered:f,paused:p,seeking:R,ended:ie,duration:se}),o=window.setTimeout(()=>{if(n){o=void 0;return}s(_.RegularInterval)},1e3)}}function N(r,e,t){if(e?.contentId!==r.value.contentId){a.info("API: Ignoring MediaSource attachment due to wrong `contentId`");return}if(e.stopPlaybackObservations!==null&&(e.stopPlaybackObservations(),e.stopPlaybackObservations=null),r.value.handle!==void 0)t.srcObject=r.value.handle;else if(r.value.src!==void 0)t.src=r.value.src;else throw new Error('Unexpected "attach-media-source" message: missing source');e.mediaSourceId=r.value.mediaSourceId,e.mediaSource=null,e.disposeMediaSource=()=>{r.value.src!==void 0&&URL.revokeObjectURL(r.value.src)},e.sourceBuffers=[],e.stopPlaybackObservations=null}function q(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){a.info("API: Ignoring seek due to wrong `mediaSourceId`");return}try{t.currentTime=r.value.position}catch(n){a.error("Unexpected error while seeking:",n)}}function V(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){a.info("API: Ignoring playback rate update due to wrong `mediaSourceId`");return}try{t.playbackRate=r.value.playbackRate}catch(n){a.error("Unexpected error while changing the playback rate:",n)}}function z(r,e,t,n){if(e?.contentId!==r.value.contentId)a.info("API: Ignoring MediaSource attachment due to wrong `contentId`");else{let{mediaSourceId:o}=r.value;try{e.disposeMediaSource?.(),e.stopPlaybackObservations?.();let i=new MediaSource,s=pe(n,i,t,o);e.mediaSourceId=r.value.mediaSourceId,e.mediaSource=i,e.disposeMediaSource=s,e.sourceBuffers=[],e.stopPlaybackObservations=null}catch(i){let{name:s,message:u}=h(i,"Unknown error when creating the MediaSource");c(n,{type:"create-media-source-error",value:{mediaSourceId:o,message:u,name:s}})}}}function H(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){a.info("API: Ignoring duration update due to wrong `mediaSourceId`");return}try{e.mediaSource===null?a.info("API: Ignoring duration update due to no MediaSource"):e.mediaSource.duration=r.value.duration}catch(n){let{name:o,message:i}=h(n,"Unknown error when updating the MediaSource's duration"),{mediaSourceId:s}=r.value;c(t,{type:"update-media-source-duration-error",value:{mediaSourceId:s,message:i,name:o}})}}function F(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){a.info("API: Ignoring MediaSource clearing due to wrong `mediaSourceId`");return}try{e.disposeMediaSource?.(),U(t)}catch(n){a.warn("API: Error when clearing current MediaSource:",n)}}function Q(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){a.info("API: Ignoring SourceBuffer creation due to wrong `mediaSourceId`");return}if(e.mediaSource===null){c(t,{type:"create-source-buffer-error",value:{mediaSourceId:r.value.mediaSourceId,sourceBufferId:r.value.sourceBufferId,code:0,message:"No MediaSource created on the main thread.",name:void 0}});return}try{let n=e.mediaSource.addSourceBuffer(r.value.contentType),o=new E(n);e.sourceBuffers.push({sourceBufferId:r.value.sourceBufferId,queuedSourceBuffer:o})}catch(n){let{name:o,message:i}=h(n,"Unknown error when adding the SourceBuffer to the MediaSource");c(t,{type:"create-source-buffer-error",value:{mediaSourceId:r.value.mediaSourceId,sourceBufferId:r.value.sourceBufferId,code:1,message:i,name:o}})}}function D(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){a.info("API: Ignoring appendBuffer operation due to wrong `mediaSourceId`");return}let n=e.sourceBuffers.find(({sourceBufferId:o})=>o===r.value.sourceBufferId);if(n!==void 0){let s=function(u){let{name:f,message:l}=h(u,"Unknown error when appending data to the SourceBuffer");c(t,{type:"source-buffer-error",value:{sourceBufferId:i,message:l,name:f}})},{mediaSourceId:o,sourceBufferId:i}=r.value;try{n.queuedSourceBuffer.push(r.value.data).then(()=>{c(t,{type:"source-buffer-updated",value:{mediaSourceId:o,sourceBufferId:i}})}).catch(s)}catch(u){s(u)}}}function G(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){a.info("API: Ignoring removeBuffer operation due to wrong `mediaSourceId`");return}let n=e.sourceBuffers.find(({sourceBufferId:o})=>o===r.value.sourceBufferId);if(n!==void 0){let s=function(u){let{name:f,message:l}=h(u,"Unknown error when removing data to the SourceBuffer");c(t,{type:"source-buffer-error",value:{sourceBufferId:i,message:l,name:f}})},{mediaSourceId:o,sourceBufferId:i}=r.value;try{n.queuedSourceBuffer.removeBuffer(r.value.start,r.value.end).then(()=>{c(t,{type:"source-buffer-updated",value:{mediaSourceId:o,sourceBufferId:i}})}).catch(s)}catch(u){s(u)}}}function $(r,e,t,n){if(e?.mediaSourceId!==r.value.mediaSourceId){a.info("API: Ignoring `start-playback-observation` due to wrong `mediaSourceId`");return}e.stopPlaybackObservations!==null&&(e.stopPlaybackObservations(),e.stopPlaybackObservations=null),e.stopPlaybackObservations=O(t,r.value.mediaSourceId,o=>c(n,{type:"observation",value:o}))}function X(r,e){if(e?.mediaSourceId!==r.value.mediaSourceId){a.info("API: Ignoring `stop-playback-observation` due to wrong `mediaSourceId`");return}e.stopPlaybackObservations!==null&&(e.stopPlaybackObservations(),e.stopPlaybackObservations=null)}function J(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId)a.info("API: Ignoring `end-of-stream` due to wrong `mediaSourceId`");else{let{mediaSourceId:n}=r.value;if(e.mediaSource===null){c(t,{type:"end-of-stream-error",value:{mediaSourceId:n,code:0,message:"No MediaSource created on the main thread.",name:void 0}});return}if(e.mediaSource.readyState==="ended"){a.info("Ignoring redundant end-of-stream order");return}try{e.mediaSource.endOfStream()}catch(o){let{name:i,message:s}=h(o,"Unknown error when calling MediaSource.endOfStream()");c(t,{type:"end-of-stream-error",value:{mediaSourceId:n,code:1,message:s,name:i}})}}}function K(r,e){if(e?.contentId!==r.value.contentId){a.info("API: Ignoring media offset update due to wrong `contentId`");return}e.mediaOffset=r.value.offset}function Y(r,e,t){return e?.mediaSourceId!==r.value.mediaSourceId?(a.info("API: Ignoring rebuffering start due to wrong `mediaSourceId`"),!1):(r.value.updatePlaybackRate&&(t.playbackRate=0),e.isRebuffering?!1:(e.isRebuffering=!0,!0))}function Z(r,e,t){return e?.mediaSourceId!==r.value.mediaSourceId?(a.info("API: Ignoring rebuffering end due to wrong `mediaSourceId`"),!1):(t.playbackRate===0&&e.wantedSpeed!==0&&(t.playbackRate=e.wantedSpeed),e.isRebuffering?(e.isRebuffering=!1,!0):!1)}function ee(r,e){if(e?.contentId!==r.value.contentId)return a.info("API: Ignoring error due to wrong `contentId`"),null;e.loadingAborter!==void 0&&e.loadingAborter.abort(new Error("Could not load content due to an error")),e.disposeMediaSource?.(),e.stopPlaybackObservations?.();let t;switch(r.value.errorInfo.type){case"segment-request":t=new y(r.value.errorInfo.value,r.value.message);break;case"other-error":t=new M(r.value.errorInfo.value.code,r.value.message);break;case"source-buffer-creation-error":t=new b(r.value.errorInfo.value.code,r.value.message);break;case"playlist-parse":t=new P(r.value.errorInfo.value.type,r.value.errorInfo.value.mediaType,r.value.message);break;default:t=new Error(r.value.message??"An error arised");break}return e.error=t,t}function re(r,e){return e?.contentId!==r.value.contentId&&a.info("API: Ignoring warning due to wrong `contentId`"),null}function te(r,e){if(e?.contentId!==r.value.contentId){a.info("API: Ignoring warning due to wrong `contentId`");return}e.minimumPosition=r.value.minimumPosition,e.maximumPosition=r.value.maximumPosition}function ne(r,e){return e?.contentId!==r.value.contentId?(a.info("API: Ignoring warning due to wrong `contentId`"),!1):(e.variants=r.value.variants,!0)}function oe(r,e){if(e?.contentId!==r.value.contentId)return a.info("API: Ignoring warning due to wrong `contentId`"),!1;let t=e.variants.find(n=>n.id===r.value.variantId);return t===void 0&&a.warn("API: VariantUpdate for an unfound variant"),t!==e.currVariant?(e.currVariant=t,!0):!1}function ae(r,e){return e?.contentId!==r.value.contentId?(a.info("API: Ignoring `content-stopped` due to wrong `contentId`"),!1):(e.disposeMediaSource?.(),e.stopPlaybackObservations?.(),e.loadingAborter?.abort(new Error("Content Stopped")),!0)}function pe(r,e,t,n){e.addEventListener("sourceclose",u),e.addEventListener("sourceended",i),e.addEventListener("sourceopen",s);let o=URL.createObjectURL(e);t.src=o;function i(){c(r,{type:"media-source-state-changed",value:{mediaSourceId:n,state:w.Ended}})}function s(){c(r,{type:"media-source-state-changed",value:{mediaSourceId:n,state:w.Open}})}function u(){c(r,{type:"media-source-state-changed",value:{mediaSourceId:n,state:w.Closed}})}return()=>{if(e.removeEventListener("sourceclose",u),e.removeEventListener("sourceended",i),e.removeEventListener("sourceopen",s),URL.revokeObjectURL(o),e.readyState!=="closed"){let{readyState:f,sourceBuffers:l}=e;for(let d=l.length-1;d>=0;d--){let p=l[d];if(!p.updating)try{f==="open"&&p.abort(),e.removeSourceBuffer(p)}catch{}}}t.src="",t.removeAttribute("src")}}var ge=W();var me={bufferGoal:30,segmentRequestTimeout:2e4,segmentBackoffBase:300,segmentBackoffMax:2e3,multiVariantPlaylistRequestTimeout:15e3,multiVariantPlaylistBackoffBase:300,multiVariantPlaylistBackoffMax:2e3,mediaPlaylistRequestTimeout:15e3,mediaPlaylistBackoffBase:300,mediaPlaylistBackoffMax:2e3},I=class extends m{constructor(t,n){super();this.videoElement=t,this.initializationStatus="Uninitialized",this.__worker__=null,this.__contentMetadata__=null,this.__logLevelChangeListener__=null,this.__destroyAbortController__=new AbortController,this.__config__={...me,...n??{}};let o=()=>{this.getPlayerState()==="Loaded"&&this.trigger("paused",null)},i=()=>{this.getPlayerState()==="Loaded"&&this.trigger("ended",null)},s=()=>{this.getPlayerState()==="Loaded"&&this.__contentMetadata__!==null&&this.trigger("playing",null)};this.videoElement.addEventListener("pause",o),this.videoElement.addEventListener("play",s),this.videoElement.addEventListener("ended",i),this.__destroyAbortController__.signal.addEventListener("abort",()=>{this.videoElement.removeEventListener("pause",o),this.videoElement.removeEventListener("play",s),this.videoElement.removeEventListener("ended",i)})}initialize(t){try{if(this.initializationStatus!=="Uninitialized")throw new Error("WaspHlsPlayer already initialized");this.initializationStatus="Initializing";let{wasmUrl:n,workerUrl:o}=t,i=g,s=g,u=new Promise((f,l)=>{i=f,s=l});return this.__startWorker__(o,n,i,s),u}catch(n){return this.initializationStatus="errorred",Promise.reject(n)}}getConfig(){return this.__config__}updateConfig(t){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");this.__config__={...this.__config__,...t},c(this.__worker__,{type:"update-config",value:this.__config__})}loadContent(t){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");this.__contentMetadata__!==null&&T(this.__contentMetadata__,this.__worker__);let n=ge(),o=new AbortController;this.__contentMetadata__={contentId:n,mediaSourceId:null,mediaSource:null,disposeMediaSource:null,sourceBuffers:[],variants:[],currVariant:void 0,lockedVariant:null,stopPlaybackObservations:null,isRebuffering:!1,mediaOffset:void 0,wantedSpeed:1,minimumPosition:void 0,maximumPosition:void 0,loadingAborter:o,error:null},this.trigger("loading",null),c(this.__worker__,{type:"load",value:{contentId:n,url:t}})}getPlayerState(){return this.__contentMetadata__===null?"Stopped":this.__contentMetadata__.error!==null?"Error":this.__contentMetadata__.loadingAborter!==void 0?"Loading":"Loaded"}getPosition(){return this.__contentMetadata__===null?0:this.videoElement.currentTime-(this.__contentMetadata__.mediaOffset??0)}seek(t){if(this.getPlayerState()!=="Loaded")throw new Error("Cannot seek: no content loaded.");this.videoElement.currentTime=t+(this.__contentMetadata__?.mediaOffset??0)}getMediaOffset(){return this.__contentMetadata__?.mediaOffset??void 0}setVolume(t){this.videoElement.volume=t}isPlaying(){return this.getPlayerState()==="Loaded"&&!this.videoElement.paused}isPaused(){return this.videoElement.paused}isEnded(){return this.videoElement.ended}isRebuffering(){return this.__contentMetadata__?.isRebuffering??!1}pause(){if(this.getPlayerState()!=="Loaded")throw new Error("Cannot pause: no content loaded.");this.videoElement.pause()}resume(){if(this.getPlayerState()!=="Loaded")throw new Error("Cannot resume: no content loaded.");this.videoElement.play().catch(()=>{})}stop(){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");this.__contentMetadata__!==null&&T(this.__contentMetadata__,this.__worker__)}setSpeed(t){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");if(this.__contentMetadata__===null||this.__contentMetadata__.mediaSourceId===null)throw new Error("No content is loaded");this.__contentMetadata__.wantedSpeed=t,c(this.__worker__,{type:"update-wanted-speed",value:{mediaSourceId:this.__contentMetadata__.mediaSourceId,wantedSpeed:t}})}getSpeed(){return this.__contentMetadata__?.wantedSpeed??1}getMinimumPosition(){return this.__contentMetadata__?.minimumPosition}getMaximumPosition(){return this.__contentMetadata__?.maximumPosition}getError(){return this.__contentMetadata__?.error??null}getCurrentVariant(){return this.__contentMetadata__?.currVariant??void 0}getVariantsList(){return this.__contentMetadata__?.variants??[]}lockVariant(t){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");if(this.__contentMetadata__===null)throw new Error("No content loaded");this.__contentMetadata__.lockedVariant=t,c(this.__worker__,{type:"lock-variant",value:{contentId:this.__contentMetadata__.contentId,variantId:t}})}unlockVariant(){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");if(this.__contentMetadata__===null)throw new Error("No content loaded");this.__contentMetadata__.lockedVariant=null,c(this.__worker__,{type:"lock-variant",value:{contentId:this.__contentMetadata__.contentId,variantId:null}})}getLockedVariant(){return this.__contentMetadata__?.lockedVariant??null}dispose(){this.__destroyAbortController__.abort(),this.__worker__!==null&&(this.__contentMetadata__!==null&&T(this.__contentMetadata__,this.__worker__),c(this.__worker__,{type:"dispose",value:null}),this.__worker__=null,this.__logLevelChangeListener__!==null&&a.removeEventListener("onLogLevelChange",this.__logLevelChangeListener__),this.videoElement.src="")}__startWorker__(t,n,o,i){let s=!0,u=new Worker(t);this.__worker__=u,c(u,{type:"init",value:{hasWorkerMse:typeof MediaSource=="function"&&MediaSource.canConstructInDedicatedWorker===!0,wasmUrl:n,logLevel:a.getLevel(),initialConfig:this.__config__}}),this.__logLevelChangeListener__!==null&&a.removeEventListener("onLogLevelChange",this.__logLevelChangeListener__),a.addEventListener("onLogLevelChange",f),this.__logLevelChangeListener__=f,u.onmessage=l=>{let{data:d}=l;if(typeof d!="object"||d===null||typeof d.type!="string"){a.error("unexpected Worker message");return}switch(d.type){case"initialized":this.initializationStatus="Initialized",s=!1,o();break;case"initialization-error":if(s){let p=new v(d.value.code,d.value.wasmHttpStatus,d.value.message??"Error while initializing the WaspHlsPlayer");s=!1,i(p)}break;case"seek":q(d,this.__contentMetadata__,this.videoElement);break;case"update-playback-rate":V(d,this.__contentMetadata__,this.videoElement);break;case"attach-media-source":N(d,this.__contentMetadata__,this.videoElement),this.__startListeningToLoadedEvent__();break;case"create-media-source":z(d,this.__contentMetadata__,this.videoElement,u),this.__startListeningToLoadedEvent__();break;case"update-media-source-duration":H(d,this.__contentMetadata__,u);break;case"clear-media-source":F(d,this.__contentMetadata__,this.videoElement);break;case"create-source-buffer":Q(d,this.__contentMetadata__,u);break;case"append-buffer":D(d,this.__contentMetadata__,u);break;case"remove-buffer":G(d,this.__contentMetadata__,u);break;case"start-playback-observation":$(d,this.__contentMetadata__,this.videoElement,u);break;case"stop-playback-observation":X(d,this.__contentMetadata__);break;case"end-of-stream":J(d,this.__contentMetadata__,u);break;case"media-offset-update":K(d,this.__contentMetadata__);break;case"multivariant-parsed":ne(d,this.__contentMetadata__)&&this.trigger("variantsListUpdate",this.getVariantsList());break;case"variant-update":oe(d,this.__contentMetadata__)&&this.trigger("variantUpdate",this.getCurrentVariant());break;case"error":{let p=ee(d,this.__contentMetadata__);p!==null&&(a.error("API: sending fatal error",p),this.trigger("error",p));break}case"warning":{let p=re(d,this.__contentMetadata__);p!==null&&this.trigger("warning",p);break}case"content-time-update":te(d,this.__contentMetadata__);break;case"content-stopped":ae(d,this.__contentMetadata__)&&(this.__contentMetadata__=null,this.trigger("stopped",null));break;case"rebuffering-started":Y(d,this.__contentMetadata__,this.videoElement)&&this.trigger("rebufferingStarted",null);break;case"rebuffering-ended":Z(d,this.__contentMetadata__,this.videoElement)&&this.trigger("rebufferingEnded",null);break}},u.onerror=l=>{a.error("API: Worker Error encountered",l.error),s&&i(l.error),this.dispose()};function f(l){c(u,{type:"update-logger-level",value:l})}}__startListeningToLoadedEvent__(){let t=this.__contentMetadata__;t!==null&&t.loadingAborter!==void 0&&j(this.videoElement,t.loadingAborter.signal).then(()=>{this.__contentMetadata__!==null&&(this.__contentMetadata__.loadingAborter=void 0),this.trigger("loaded",null)},n=>{this.__contentMetadata__!==null&&(this.__contentMetadata__.loadingAborter=void 0),a.info("Could not load content:",n)})}};var ot=I;})();
