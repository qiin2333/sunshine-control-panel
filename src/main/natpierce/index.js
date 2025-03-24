// import edge from 'edge-js'
// import { join } from 'path'

// const natpierce = edge.func({
//   assemblyFile: join(__dirname, '/lib/natpierce.dll'),
//   typeName: 'NatPierce.MainClass',
//   methodName: 'Initialize',
// })

// function launchNatPierceWithPort(port) {
//   if (typeof port !== 'number' || port < 0 || port > 65535) {
//     return false
//   }

//   try {
//     const result = natpierce.Run(port, true) // 同步调用
//     return result
//   } catch (error) {
//     console.error('调用 natpierce.dll 失败:', error)
//     return false
//   }
// }

// // 注册IPC处理程序
// function registerNatPierceHandlers() {
//   ipcMain.handle('natpierce:launch', async (_, port) => await launchNatPierceWithPort(port))
// }

// export { launchNatPierceWithPort, registerNatPierceHandlers }
