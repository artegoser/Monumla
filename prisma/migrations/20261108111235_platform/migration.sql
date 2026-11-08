-- CreateTable
CREATE TABLE "User" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "username" TEXT NOT NULL,
    "usernameKey" TEXT NOT NULL,
    "email" TEXT NOT NULL,
    "passwordHash" TEXT NOT NULL,
    "country" TEXT NOT NULL DEFAULT '',
    "bio" TEXT NOT NULL DEFAULT '',
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateTable
CREATE TABLE "Session" (
    "tokenHash" TEXT NOT NULL PRIMARY KEY,
    "userId" TEXT NOT NULL,
    "expiresAt" DATETIME NOT NULL,
    CONSTRAINT "Session_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "Rating" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "userId" TEXT NOT NULL,
    "ruleset" TEXT NOT NULL,
    "category" TEXT NOT NULL,
    "value" INTEGER NOT NULL DEFAULT 1500,
    "games" INTEGER NOT NULL DEFAULT 0,
    "wins" INTEGER NOT NULL DEFAULT 0,
    "draws" INTEGER NOT NULL DEFAULT 0,
    "losses" INTEGER NOT NULL DEFAULT 0,
    CONSTRAINT "Rating_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "Game" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "ruleset" TEXT NOT NULL,
    "category" TEXT NOT NULL,
    "initialSeconds" INTEGER NOT NULL,
    "increment" INTEGER NOT NULL,
    "rated" BOOLEAN NOT NULL DEFAULT false,
    "computerLevel" INTEGER,
    "status" TEXT NOT NULL DEFAULT 'active',
    "result" TEXT,
    "termination" TEXT,
    "firstUserId" TEXT,
    "secondUserId" TEXT,
    "firstRating" INTEGER NOT NULL DEFAULT 1500,
    "secondRating" INTEGER NOT NULL DEFAULT 1500,
    "firstDelta" INTEGER,
    "secondDelta" INTEGER,
    "firstMs" INTEGER NOT NULL,
    "secondMs" INTEGER NOT NULL,
    "activeSide" INTEGER NOT NULL DEFAULT 1,
    "turnStartedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "snapshot" TEXT NOT NULL,
    "revision" INTEGER NOT NULL DEFAULT 0,
    "drawOffer" INTEGER,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "finishedAt" DATETIME,
    CONSTRAINT "Game_firstUserId_fkey" FOREIGN KEY ("firstUserId") REFERENCES "User" ("id") ON DELETE SET NULL ON UPDATE CASCADE,
    CONSTRAINT "Game_secondUserId_fkey" FOREIGN KEY ("secondUserId") REFERENCES "User" ("id") ON DELETE SET NULL ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "GameMove" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "gameId" TEXT NOT NULL,
    "ply" INTEGER NOT NULL,
    "actor" INTEGER NOT NULL,
    "notation" TEXT NOT NULL,
    "firstMs" INTEGER NOT NULL,
    "secondMs" INTEGER NOT NULL,
    "viewOne" TEXT NOT NULL,
    "viewTwo" TEXT NOT NULL,
    CONSTRAINT "GameMove_gameId_fkey" FOREIGN KEY ("gameId") REFERENCES "Game" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "Friendship" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "pairKey" TEXT NOT NULL,
    "senderId" TEXT NOT NULL,
    "recipientId" TEXT NOT NULL,
    "status" TEXT NOT NULL DEFAULT 'pending',
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "Friendship_senderId_fkey" FOREIGN KEY ("senderId") REFERENCES "User" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "Friendship_recipientId_fkey" FOREIGN KEY ("recipientId") REFERENCES "User" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "Challenge" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "senderId" TEXT NOT NULL,
    "recipientId" TEXT NOT NULL,
    "ruleset" TEXT NOT NULL,
    "initialSeconds" INTEGER NOT NULL,
    "increment" INTEGER NOT NULL,
    "rated" BOOLEAN NOT NULL DEFAULT true,
    "senderSide" INTEGER NOT NULL DEFAULT 1,
    "status" TEXT NOT NULL DEFAULT 'pending',
    "gameId" TEXT,
    "expiresAt" DATETIME NOT NULL,
    "createdAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "Challenge_senderId_fkey" FOREIGN KEY ("senderId") REFERENCES "User" ("id") ON DELETE CASCADE ON UPDATE CASCADE,
    CONSTRAINT "Challenge_recipientId_fkey" FOREIGN KEY ("recipientId") REFERENCES "User" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "QueueEntry" (
    "userId" TEXT NOT NULL PRIMARY KEY,
    "ruleset" TEXT NOT NULL,
    "category" TEXT NOT NULL,
    "initialSeconds" INTEGER NOT NULL,
    "increment" INTEGER NOT NULL,
    "rated" BOOLEAN NOT NULL,
    "rating" INTEGER NOT NULL,
    "joinedAt" DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT "QueueEntry_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateIndex
CREATE UNIQUE INDEX "User_usernameKey_key" ON "User"("usernameKey");

-- CreateIndex
CREATE UNIQUE INDEX "User_email_key" ON "User"("email");

-- CreateIndex
CREATE INDEX "Session_userId_idx" ON "Session"("userId");

-- CreateIndex
CREATE INDEX "Session_expiresAt_idx" ON "Session"("expiresAt");

-- CreateIndex
CREATE INDEX "Rating_ruleset_category_value_idx" ON "Rating"("ruleset", "category", "value");

-- CreateIndex
CREATE UNIQUE INDEX "Rating_userId_ruleset_category_key" ON "Rating"("userId", "ruleset", "category");

-- CreateIndex
CREATE INDEX "Game_firstUserId_createdAt_idx" ON "Game"("firstUserId", "createdAt");

-- CreateIndex
CREATE INDEX "Game_secondUserId_createdAt_idx" ON "Game"("secondUserId", "createdAt");

-- CreateIndex
CREATE INDEX "Game_status_createdAt_idx" ON "Game"("status", "createdAt");

-- CreateIndex
CREATE UNIQUE INDEX "GameMove_gameId_ply_key" ON "GameMove"("gameId", "ply");

-- CreateIndex
CREATE UNIQUE INDEX "Friendship_pairKey_key" ON "Friendship"("pairKey");

-- CreateIndex
CREATE INDEX "Challenge_recipientId_status_expiresAt_idx" ON "Challenge"("recipientId", "status", "expiresAt");

-- CreateIndex
CREATE INDEX "QueueEntry_ruleset_initialSeconds_increment_rated_joinedAt_idx" ON "QueueEntry"("ruleset", "initialSeconds", "increment", "rated", "joinedAt");
